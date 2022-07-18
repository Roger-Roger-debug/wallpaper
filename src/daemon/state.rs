#![warn(missing_docs)]
use log::{info, trace};
use rand::Rng;
use std::{collections::VecDeque, fs, path::PathBuf, process::Command, time::Duration};

use crate::WallpaperMethod;

/// Global object to store the current state
#[derive(Debug)]
pub struct State {
    history: VecDeque<PathBuf>, // Never empty
    action: NextImage,
    previous_action: NextImage,
    change_interval: Duration,
    image_dir: PathBuf,
    use_fallback: bool,
    default_image: PathBuf,
    wallpaper_cmd: WallpaperMethod,
}

#[derive(Debug, Clone, PartialEq, Eq, Copy)]
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
        wallpaper_cmd: WallpaperMethod,
    ) -> Self {
        let mut history = VecDeque::new();
        history.push_back(default_image.clone());

        let state = State {
            history,
            action: NextImage::Static,
            previous_action: NextImage::Static,
            change_interval,
            image_dir,
            use_fallback: false,
            default_image,
            wallpaper_cmd,
        };
        trace!("Construction state:\n{:?}", state);
        state.update();
        state
    }

    pub fn change_image(&mut self, direction: ChangeImageDirection) {
        if let NextImage::Static = self.action {
            return;
        }

        match direction {
            ChangeImageDirection::Next => {
                info!("Going to the next image");
                // If not enough space delete one element
                if self.history.len() >= 25 {
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
            ChangeImageDirection::Previous => {
                info!("Going to the previous image");
                if self.history.len() > 1 {
                    info!("There is no previous image");
                    self.history.pop_back();
                }
            }
        }

        self.update();
    }

    pub fn update(&self) {
        trace!("Updating current wallpaper");
        let path = self.history.back().unwrap().clone();
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
                trace!("Current stack {:?}", self.history);
                trace!("setting wallpaper for hyprpaper {}", path.to_string_lossy());
                // preload image
                let output = Command::new("hyprctl")
                    .args(["hyprpaper", "preload"])
                    .arg(format!("{}", path.to_string_lossy()))
                    .output();
                info!("{output:?}");
                // set image
                args.list.iter().for_each(|monitor| {
                    trace!("Setting wallpaper for monitor {monitor}");
                    let output = Command::new("hyprctl")
                        .args(["hyprpaper", "wallpaper"])
                        .arg(format!("{monitor},{}", path.to_string_lossy()))
                        .output();
                    info!("{output:?}");
                });
                // unload old image
                if self.history.len() > 2 {
                    let prev = self.history.iter().rev().nth(2).unwrap();
                    trace!(
                        "unloading wallpaper for hyprpaper {}",
                        prev.to_string_lossy()
                    );
                    //thread::sleep(Duration::from_secs(1));
                    let output = Command::new("hyprctl")
                        .args(["hyprpaper", "unload"])
                        .arg(format!("{}", prev.to_string_lossy()))
                        .output();
                    info!("{output:?}");
                }
            }
        }
    }

    pub fn update_image(&mut self, action: NextImage, image: Option<PathBuf>) {
        info!("Setting action to {:?}", action);
        if self.history.len() >= 25 && image.is_some() {
            self.history.pop_front();
        }
        self.previous_action = self.action;
        self.action = action;
        if image.is_some() {
            self.history.push_back(image.unwrap());
            self.update();
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
            self.history.pop_back();
        }
        self.update();
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
}
