use log::info;
use rand::{prelude::SliceRandom, thread_rng};
use std::{path::PathBuf, process::Command, time::Duration};

use crate::Action;

pub struct State {
    action: Action,
    old_action: Action,
    change_interval: Duration,
    path: String,
    images: Vec<PathBuf>,
    index: usize,
    use_fallback: bool,
    default: PathBuf,
}

impl State {
    pub fn new(change_interval: Duration, path: String, default: PathBuf) -> Self {
        let images = State::get_images(&path);
        let state = State {
            action: Action::Static(Some(default.clone())),
            old_action: Action::Static(Some(default.clone())),
            change_interval,
            path,
            images,
            index: 0,
            use_fallback: false,
            default,
        };
        state.update();
        state
    }

    pub fn get_images(path: &str) -> Vec<PathBuf> {
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
        let path = match &self.action {
            Action::Static(path) => path.as_ref().and_then(|path| {
                if std::path::Path::new(path).exists() {
                    Some(path)
                } else {
                    None
                }
            }),
            _ => {
                let file = self.images.get(self.index).unwrap();
                Some(file)
            }
        };
        if let Some(path) = path {
            info!("Changing background to {:?}", path);
            let mut process = Command::new("feh")
                .arg("--bg-scale")
                .arg(path)
                .spawn()
                .unwrap();
            process.wait().unwrap();
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
            self.action = Action::Static(Some(self.default.clone()));
            self.update();
        } else {
            self.action = self.old_action.clone();
            self.next();
        }
    }

    pub fn update_dir(&mut self) {
        self.images = Self::get_images(&self.path);
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
