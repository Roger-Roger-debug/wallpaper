use log::{info, trace};
use rand::{prelude::SliceRandom, thread_rng};
use std::{
    path::{Path, PathBuf},
    process::Command,
    time::Duration,
};

use crate::{Action, WallpaperMethod};

#[derive(Debug)]
pub struct State {
    action: Action,
    old_action: Action,
    change_interval: Duration,
    image_dir: PathBuf,
    images: Vec<PathBuf>,
    index: usize,
    use_fallback: bool,
    default_image: PathBuf,
    wallpaper_cmd: WallpaperMethod,
}

impl State {
    pub fn new(
        change_interval: Duration,
        image_dir: PathBuf,
        default: PathBuf,
        wallpaper_cmd: WallpaperMethod,
    ) -> Self {
        trace!(
            "New state constructed with {:?}, {:?}, {:?}, {:?}",
            change_interval,
            image_dir,
            default,
            wallpaper_cmd
        );
        let images = State::get_images(&image_dir);
        let state = State {
            action: Action::Static(Some(default.clone())),
            old_action: Action::Static(Some(default.clone())),
            change_interval,
            image_dir,
            images,
            index: 0,
            use_fallback: false,
            default_image: default,
            wallpaper_cmd,
        };
        trace!("State is {:?}", state);
        state.update();
        state
    }

    pub fn get_images(path: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(path)
            .unwrap() //read dir
            .into_iter()
            .map(|item| {
                item.unwrap().path() //convert to abs. path
            })
            .collect::<Vec<PathBuf>>()
    }

    pub fn next(&mut self) {
        if let Action::Static(_) = self.action {
            return;
        }
        self.index += 1;
        self.index %= self.images.len();
        info!("Going to next image");

        if let Action::Random = self.action {
            self.shuffle();
        }

        self.update();
    }

    pub fn shuffle(&mut self) {
        self.images.shuffle(&mut thread_rng());
        info!("Shuffle");
    }

    pub fn prev(&mut self) {
        info!("Going to previous image");
        if self.index == 0 {
            self.index = self.images.len() - 1;
        } else {
            self.index -= 1;
        }

        self.update();
    }

    pub fn update(&self) {
        trace!("Action is {:?}", self.action);
        let path = match &self.action {
            Action::Static(path) => path.as_ref().and_then(|path| {
                if std::path::Path::new(path).exists() {
                    trace!("Static image with path {}", path.to_string_lossy());
                    Some(path)
                } else {
                    trace!("Static image without path");
                    None
                }
            }),
            _ => {
                let file = self.images.get(self.index).unwrap();
                trace!("Non static image set to {}", file.to_string_lossy());
                Some(file)
            }
        };
        if let Some(path) = path {
            info!("Changing background to {:?}", path);
            self.set_wallpaper(path);
        }
    }

    fn set_wallpaper(&self, path: &PathBuf) {
        trace!("Reached set_wallpaper");
        match &self.wallpaper_cmd {
            WallpaperMethod::Feh => {
                let mut process = Command::new("feh")
                    .arg("--bg-scale")
                    .arg(path)
                    .spawn()
                    .unwrap();
                process.wait().unwrap();
            }
            WallpaperMethod::Hyprpaper(args) => match &args.monitor {
                //crate::HyprpaperMonitor::All => todo!(),
                crate::HyprpaperMonitor::List { list } => {
                    trace!("setting wallpaper for hyprpaper");
                    // preload image
                    let output = Command::new("hyprctl")
                        .args(["hyprpaper", "preload"])
                        .arg(format!("{}", path.to_string_lossy()))
                        .output();
                    info!("{output:?}");
                    list.iter().for_each(|monitor| {
                        trace!("Setting wallpaper for monitor {monitor}");
                        let output = Command::new("hyprctl")
                            .args(["hyprpaper", "wallpaper"])
                            .arg(format!("{monitor},{}", path.to_string_lossy()))
                            .output();
                        info!("{output:?}");
                    });
                }
            },
        }
    }

    pub fn update_action(&mut self, action: Action, update: bool) {
        info!("Setting action to {:?}", action);
        self.action = action;
        if update {
            self.update();
        }
    }

    pub fn save(&mut self) {
        self.use_fallback = !self.use_fallback;
        info!("Setting fallback to {}", self.use_fallback);
        if self.use_fallback {
            self.old_action = self.action.clone();
            self.action = Action::Static(Some(self.default_image.clone()));
            trace!("New action is {:?}", self.action);
            self.update();
        } else {
            self.action = self.old_action.clone();
            self.next();
        }
    }

    pub fn update_dir(&mut self) {
        self.images = Self::get_images(&self.image_dir);
    }

    pub fn get_current_image(&self) -> &PathBuf {
        &self.images[self.index]
    }

    pub fn get_action(&self) -> Action {
        self.action.clone()
    }

    pub fn change_interval(&mut self, i: Duration) {
        self.change_interval = i;
    }

    pub fn get_change_interval(&self) -> Duration {
        self.change_interval
    }
}
