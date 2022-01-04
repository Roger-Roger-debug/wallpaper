use std::convert::TryFrom;
use std::fs;
use std::io::Read;
use std::os::unix::net::*;
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use log::info;

mod args;
mod state;

use state::State;

use clap::{app_from_crate, arg};

const HELP: &str = "Usage:
    wp <option> [argument]

Options:
    help: show this help
    stop: stop the server
    next: show the next image
    prev: show the previous image
    rng: change mode to random
    linear: change mode to linear
    hold: don't change the current wallpaper
    update: update image folder
    save: no horni
    shuffle: shuffle wallpaper queue
    interval: get the current interval
    get: get [argument]

Arguments:
    wp | wallpaper: get the current wallpaper
    ac | actions: get the current action
    dur| duration: get the current interval";

//TODO: error handling
#[derive(Debug, Clone)]
pub enum Action {
    Random,
    Linear,
    Static(Option<PathBuf>),
}

fn main() {
    pretty_env_logger::init();

    let matches = app_from_crate!()
        .arg(arg!(-f --fallback <PATH> "required field that specifies which image to desplay as a fallback").required(true))
        .arg(arg!(-p --path [PATH]))
        .arg(arg!(-s --socket [PATH]))
        //.arg(arg!(--duration <SECONDS>))
        //.arg(arg!(-a --action [ACTION]))
        .get_matches();

    let default_image_path = PathBuf::from_str(
        matches
            .value_of("fallback")
            .expect("Default image was not provided"),
    )
    .expect("Invalid imate path");
    let socket_path = match matches.value_of("socket") {
        Some(val) => PathBuf::from_str(val).expect("Invalid socket path"),
        None => {
            if let Ok(path) = std::env::var("XDG_RUNTIME_DIR") {
                PathBuf::from_str(&format!("{}/wallpaperd", path)).expect("Invalid XDG_RUNTIME_DIR")
            } else {
                PathBuf::from_str("/tmp/wallpaperd").unwrap()
            }
        }
    };

    let image_dir = match matches.value_of("path") {
        Some(val) => val.to_string(),
        None => format!("{}/Pictures/backgrounds/", std::env::var("HOME").unwrap()),
    };

    let time = Duration::new(60, 0);
    let data = Arc::new(Mutex::new(State::new(time, image_dir, default_image_path)));

    let listener = UnixListener::bind(&socket_path).unwrap();
    let mut incoming = listener.incoming();

    info!("Binding socket {:?}", socket_path);

    let d = data.clone();
    thread::spawn(move || change_interval(d));

    while let Some(stream) = incoming.next() {
        let d = data.clone();
        let handle = thread::spawn(move || handle_connection(stream.unwrap(), d));
        if let Ok(res) = handle.join() {
            if res {
                break;
            }
        }
    }

    fs::remove_file(socket_path).expect("Can't delete socket");
    exit(0);
}

fn read_from_stream(mut stream: &UnixStream) -> String {
    let string = String::new();

    //First run get length
    let mut buf: [u8; 8] = [0; 8];

    if let Err(_) = stream.read_exact(&mut buf) {
        return string;
    }

    let mut buffer = vec![0; usize::from_ne_bytes(buf)];
    if let Err(_) = stream.read_exact(&mut buffer) {
        string
    } else {
        String::from_utf8(buffer).unwrap()
    }
}

// Thread: Client <---> Server
fn handle_connection(mut stream: UnixStream, state: Arc<Mutex<State>>) -> bool {
    use std::io::prelude::*;
    info!("Handle new connection");
    let mut line = read_from_stream(&stream);
    let mut response = "".to_string();

    line = line.to_lowercase();
    info!("Got {}", &line);
    let message = args::Args::try_from(line.as_str());
    let mut stop_server = false;
    if let Ok(message) = message {
        use args::Args::*;
        use args::*;
        match message {
            Stop => stop_server = true,
            Next => state.lock().unwrap().next(),
            Prev => state.lock().unwrap().prev(),
            Help => response = HELP.to_string(),
            RNG => state.lock().unwrap().update_action(Action::Random, false),
            Linear => state.lock().unwrap().update_action(Action::Linear, false),
            Update => state.lock().unwrap().update_dir(),
            Save => state.lock().unwrap().save(),
            Shuffle => state.lock().unwrap().shuffle(),
            Hold(img) => {
                if let Some(img) = img {
                    state
                        .lock()
                        .unwrap()
                        .update_action(Action::Static(Some(img.into())), true);
                } else {
                    state
                        .lock()
                        .unwrap()
                        .update_action(Action::Static(None), false);
                }
            }
            Interval(d) => {
                state.lock().unwrap().change_interval(d);
            }
            Get(d) => {
                response = match d {
                    MessageArgs::Wallpaper => state
                        .lock()
                        .unwrap()
                        .get_current_image()
                        .clone()
                        .to_str()
                        .unwrap_or("ERROR")
                        .to_owned(),
                    MessageArgs::Action => {
                        let action = state.lock().unwrap().get_action();
                        match action {
                            Action::Linear => "Linear".to_string(),
                            Action::Static(_) => "Static".to_string(),
                            Action::Random => "Random".to_string(),
                        }
                    }
                    MessageArgs::Duration => {
                        format!("{:?} seconds", state.lock().unwrap().get_change_interval())
                    }
                }
            }
        }
    } else {
        response = "I do not understand".to_string();
    }

    stream.write(&response.as_bytes()).unwrap();
    stop_server
}

fn change_interval(data: Arc<Mutex<State>>) {
    loop {
        let time = {
            //Go out of scope to unlock again
            let mut unlocked = data.lock().unwrap();
            unlocked.next();
            unlocked.get_change_interval()
        };
        sleep(time);
    }
}
