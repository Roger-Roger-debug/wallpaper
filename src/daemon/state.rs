use std::{convert::TryInto, path::PathBuf, process::Command, time::Duration};
use rand::{prelude::SliceRandom, thread_rng};
use log::info;

use crate::Action;

pub struct State {
    action: Action,
    change_interval: Duration,
    path: String,
    images: Vec<PathBuf>,
    index: usize,
    no_horni: bool,
}

impl State {
    pub fn new(action: Action, change_interval: Duration, path: String) -> Self {
        let images = State::get_images(&path);
        State {
            action,
            change_interval,
            path,
            images,
            index: 0,
            no_horni: false,
        }
    }

    pub fn get_images(path: &str) -> Vec<PathBuf> {
        std::fs::read_dir(path).unwrap() //read dir
            .into_iter().map(|item| {
                item.unwrap().path() //convert to abs. path
            }).collect::<Vec<PathBuf>>()
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
        info!("Going to next previous");
        if self.index == 0 {
            self.index = self.images.len() - 1;
        } else {
            self.index -= 1;
        }

        self.update();
    }

    pub fn update(&self) {
        if let Action::Static(path) = &self.action {
            if let Some(path) = path {
                if std::path::Path::new(path).exists() {
                info!("Changing background to {:?}", path);
                    Command::new("feh")
                        .arg("--bg-scale")
                        .arg(path)
                        .spawn()
                        .unwrap();
                }
            }
        } else {
            let file = self.images.get(self.index).unwrap();
            info!("Changing background to {:?}", file);
            Command::new("feh")
                .arg("--bg-scale")
                .arg(&file)
                .spawn()
                .unwrap();
        }
    }

    pub fn update_action(&mut self, action: Action) {
        info!("Setting action to {:?}", action);
        self.action = action;
    }

    pub fn save(&mut self) {
        self.no_horni = !self.no_horni;
        info!("Setting horni to {}", self.no_horni);
        if self.no_horni {
            let path = self.path.clone() + "/foh0n427ez471.png";
            self.action = Action::Static(Some(path.try_into().unwrap()));
            self.update();
        } else {
            self.action = Action::Random;
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
