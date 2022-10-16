#![warn(missing_docs)]
use clap::clap_derive::ArgEnum;
use log::{error, info, trace};
use rand::Rng;
use std::{
    collections::VecDeque,
    fs,
    io::{Read, Write},
    os::unix::net::UnixStream,
    path::PathBuf,
    process::Command,
    time::Duration,
};

use crate::WallpaperMethod;

/// Global object to store the current state
#[derive(Debug)]
pub struct State {
    history: VecDeque<PathBuf>, // Never empty
    next: Vec<PathBuf>,         // Possibly empty
    history_max_size: usize,
    action: NextImage,
    previous_action: NextImage,
    change_interval: Duration,
    image_dir: PathBuf,
    use_fallback: bool,
    default_image: PathBuf,
    wallpaper_cmd: WallpaperMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy, ArgEnum)]
pub enum NextImage {
    Random,
    Linear,
    Static,
}

pub enum ChangeImageDirection {
    Next,
    Previous,
}

impl State {
    pub fn new(
        change_interval: Duration,
        image_dir: PathBuf,
        default_image: PathBuf,
        action: NextImage,
        wallpaper_cmd: WallpaperMethod,
        history_max_size: usize,
    ) -> Self {
        let mut history = VecDeque::new();
        history.push_back(default_image.clone());

        State {
            history,
            next: Vec::new(),
            history_max_size,
            action,
            previous_action: action,
            change_interval,
            image_dir,
            use_fallback: false,
            default_image,
            wallpaper_cmd,
        }
    }

    pub fn change_image(&mut self, direction: ChangeImageDirection) {
        if self.use_fallback {
            info!("Can't change image while using fallback");
            return;
        }
        if let NextImage::Static = self.action {
            info!("Can't change image while in static mode");
            return;
        }

        match direction {
            ChangeImageDirection::Next => {
                info!("Going to the next image");
                if !self.next.is_empty() {
                    self.history.push_back(self.next.pop().unwrap());
                } else {
                    // If not enough space delete one element
                    if self.history.len() >= self.history_max_size {
                        self.history.pop_front();
                    }
                    let mut idx = fs::read_dir(&self.image_dir)
                        .unwrap()
                        .position(|elem| elem.unwrap().path() == *self.history.back().unwrap())
                        .unwrap_or(0);

                    if self.action == NextImage::Random {
                        idx = rand::thread_rng()
                            .gen_range(0..fs::read_dir(&self.image_dir).unwrap().count());
                    }

                    self.history.push_back(
                        fs::read_dir(&self.image_dir).unwrap().nth(idx + 1).map_or(
                            fs::read_dir(&self.image_dir)
                                .unwrap()
                                .next()
                                .unwrap()
                                .unwrap()
                                .path(),
                            |next| next.unwrap().path(),
                        ),
                    );
                }
            }
            ChangeImageDirection::Previous => {
                info!("Going to the previous image");
                if self.history.len() > 1 {
                    self.next.push(self.history.pop_back().unwrap());
                } else {
                    info!("There is no previous image");
                }
            }
        }

        if let Err(_) = self.update() {
            error!("Error setting the wallpaper");
        }
    }

    pub fn update(&self) -> Result<(), ()> {
        info!("Updating current wallpaper");
        let path = self.get_current_image();
        trace!("setting wallpaper to {}", path.to_string_lossy());
        match &self.wallpaper_cmd {
            WallpaperMethod::Feh => {
                let mut process = Command::new("feh")
                    .arg("--bg-scale")
                    .arg(path)
                    .spawn()
                    .unwrap();
                process.wait().unwrap();
            }
            WallpaperMethod::Hyprpaper(args) => {
                // Preload the wallpaper
                self.send_to_hyprpaper(
                    format!("preload {}", self.get_current_image().to_string_lossy()).as_bytes(),
                )?;
                // Display the wallpaper on every monitor
                for monitor in args.monitors.iter() {
                    self.send_to_hyprpaper(
                        format!(
                            "wallpaper {monitor},{}",
                            self.get_current_image().to_string_lossy()
                        )
                        .as_bytes(),
                    )?;
                }

                if self.history.len() > 2 {
                    let prev = self.history.iter().rev().nth(2).unwrap();
                    if prev != self.get_current_image() {
                        self.send_to_hyprpaper(
                            format!("unload {}", prev.to_string_lossy()).as_bytes(),
                        )?;
                    }
                }
            }
        }
        Ok(())
    }

    fn send_to_hyprpaper(&self, msg: &[u8]) -> Result<String, ()> {
        let signature = std::env::var("HYPRLAND_INSTANCE_SIGNATURE").map_err(|_| ())?;
        let path: PathBuf = ["/tmp/hypr", &signature, ".hyprpaper.sock"]
            .iter()
            .collect();

        info!("Connecting to socket at {}", path.to_string_lossy());

        let mut listener = UnixStream::connect(path).map_err(|_| ())?;
        listener.write_all(msg).map_err(|_| ())?;

        listener.flush().map_err(|_| ())?;
        let mut buffer = String::new();
        listener.read_to_string(&mut buffer).map_err(|_| ())?;

        info!("Got result: {buffer}");
        Ok(buffer)
    }

    pub fn update_image(&mut self, action: NextImage, image: Option<PathBuf>) {
        info!("Setting action to {:?}", action);
        if self.history.len() >= self.history_max_size && image.is_some() {
            self.history.pop_front();
        }
        self.action = action;
        if image.is_some() {
            self.history.push_back(image.unwrap());
            if let Err(_) = self.update() {
                error!("Error setting the wallpaper");
            }
        }
    }

    pub fn save(&mut self) {
        self.use_fallback = !self.use_fallback;
        info!("Setting fallback to {}", self.use_fallback);
        if self.use_fallback {
            self.previous_action = self.action;
            self.action = NextImage::Static;
            self.history.push_back(self.default_image.clone());
        } else {
            self.action = self.previous_action;
            self.history.pop_back();
        }
        if let Err(_) = self.update() {
            error!("Error setting the wallpaper");
        }
    }

    pub fn get_current_image(&self) -> &PathBuf {
        &self.history.back().unwrap()
    }

    pub fn get_action(&self) -> NextImage {
        self.action
    }

    pub fn change_interval(&mut self, i: Duration) {
        self.change_interval = i;
    }

    pub fn get_change_interval(&self) -> Duration {
        self.change_interval
    }

    pub fn get_fallback(&self) -> bool {
        self.use_fallback
    }
}
