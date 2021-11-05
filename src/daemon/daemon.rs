use std::convert::TryInto;
use std::ffi::OsString;
use std::io::Read;
use std::mem::size_of;
use std::os::unix::net::*;
use std::process::{Command, exit};
use std::sync::mpsc::{channel, Sender};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;
use std::fs;

use rand::prelude::*;

use log::info;


//TODO: error handling
struct Stop {}

#[derive(Debug)]
enum Action {
    Random,
    Linear,
    Static(Option<OsString>),
}

struct State {
    action: Action,
    change_interval: Duration,
    path: String,
    images: Vec<OsString>,
    index: usize,
    no_horni: bool,
}

impl State {
    fn new(action: Action, change_interval: Duration, path: String) -> Self {
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

    fn get_images(path: &str) -> Vec<OsString> {
        std::fs::read_dir(path).unwrap() //read dir
            .into_iter().map(|item| {
                item.unwrap().path().into_os_string() //convert to abs. path
            }).collect::<Vec<OsString>>()
    }

    fn next(&mut self) {
        self.index += 1;
        self.index %= self.images.len();
        info!("Going to next image");

        if let Action::Random = self.action {
            self.shuffle();
        }

        self.update(false);
    }

    fn shuffle(&mut self) {
        self.images.shuffle(&mut thread_rng());
        info!("Shuffle");
    }

    fn prev(&mut self) {
        info!("Going to next previous");
        if self.index == 0 {
            self.index = self.images.len() - 1;
        } else {
            self.index -= 1;
        }

        self.update(false);
    }

    fn update(&self, force: bool) {
        if let Action::Static(path) = &self.action {
            if force {
                info!("Forcing no horni");
                Command::new("feh")
                    .arg("--bg-scale")
                    .arg(path.as_ref().unwrap()) //Don't force with none static
                    .spawn()
                    .unwrap();
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

    fn update_action(&mut self, action: Action) {
        info!("Setting action to {:?}", action);
        self.action = action;
    }

    fn save(&mut self) {
        self.no_horni = !self.no_horni;
        info!("Setting horni to {}", self.no_horni);
        if self.no_horni {
            let path = self.path.clone() + "/foh0n427ez471.png";
            self.action = Action::Static(Some(path.try_into().unwrap()));
            self.update(true);
        } else {
            self.action = Action::Random;
            self.next();
        }
    }

    fn update_dir(&mut self) {
        self.images = Self::get_images(&self.path);
    }

    fn get_current_image(&self) -> &OsString {
        &self.images[self.index]
    }
}

fn main() {
    pretty_env_logger::init();

    let socket_path = "/tmp/test.socket";

    let (tx, rx) = channel();

    thread::spawn(move|| setup(tx, socket_path));

    loop { //Don't quite the program
        let _msg = rx.recv().unwrap();
        info!("Stopping Server");
        break;
    }

    fs::remove_file(socket_path).expect("Can't delete socket");

    exit(0);
}

// Listen for incoming Connections
fn setup(main_thread: Sender<Stop>, socket_path: &str) {
    let time = Duration::new(60, 0);

    let data = Arc::new(Mutex::new(State::
                                   new(Action::Random,
                                        time, format!("{}/Pictures/backgrounds/",
                                        std::env::var("HOME").unwrap()))));

    let listener = UnixListener::bind(socket_path).unwrap();
    let mut incoming = listener.incoming();

    info!("Binding socket {}", socket_path);

    let d = data.clone();
    thread::spawn(move || change_interval(d));

    while let Some(stream) = incoming.next() {
        let tx2 = main_thread.clone();
        let d = data.clone();
        thread::spawn(move || handle_connection(stream.unwrap(), d, tx2.clone()));
    }
}

fn read_from_stream(mut stream: &UnixStream) -> String {
    use std::str;
    let mut string = String::new();

    //First run get length
    let mut buf: [u8; 8] = [0; 8];
    let mut buf_remaining: [u8; 1] = [0; 1];

    if let Err(_) = stream.read_exact(&mut buf) {
        return string
    }

    let length = usize::from_ne_bytes(buf);
    let buf_size = size_of::<usize>();

    for i in 0..=(length / buf_size) {
        if i >= length / buf_size {
            for _ in 0..(length % buf_size) {
                if let Ok(()) = stream.read_exact(&mut buf_remaining) {
                    if let Ok(str) = str::from_utf8(&buf_remaining) {
                        string.push_str(str);
                    }
                }
            }
        } else {
            if let Ok(()) = stream.read_exact(&mut buf) {
                if let Ok(str) = str::from_utf8(&buf) {
                    string.push_str(str);
                }
            }
        }
    }

    string
}

// Thread: Client <---> Server
fn handle_connection(mut stream: UnixStream, state: Arc<Mutex<State>>, main_thread: Sender<Stop>) {
    use std::io::prelude::*;
    info!("Handle new connection");
    let mut line = read_from_stream(&stream);
    let mut response = None;

    line = line.to_lowercase();
    info!("Got {}", &line);
    let splits: Vec<&str> = line.split(' ').collect();
    let tupl = (splits.get(0).unwrap_or(&"").to_owned(), splits.get(1));
    match tupl {
        ("stop", _)=> main_thread.send(Stop {}).unwrap(),
        ("exit", _) => main_thread.send(Stop {}).unwrap(),
        ("next", _) => state.lock().unwrap().next(),
        ("prev", _) => state.lock().unwrap().prev(),
        ("rng", _) => state.lock().unwrap().update_action(Action::Random),
        ("lin", _) => state.lock().unwrap().update_action(Action::Linear),
        ("hold", _) => state.lock().unwrap().update_action(Action::Static(None)),
        ("update", _) => state.lock().unwrap().update_dir(),
        ("save", _) => state.lock().unwrap().save(),
        ("shf", _) => state.lock().unwrap().shuffle(),
        ("int", d) => {
            if let Some(d) = d {
                let d = Duration::new(d.parse::<u64>().unwrap_or(60), 0);
                state.lock().unwrap().change_interval = d;
            }
        }
        ("get", _) => {
            response = Some(state.lock().unwrap().get_current_image().clone().to_str().unwrap_or("ERROR").to_owned())
        },
        _ => response = Some("Wrong argument(s)".to_owned()),
    }

    if let Some(response) = response {
        stream.write(&response.as_bytes()).unwrap();
    } else {
        stream.write(b"").unwrap();
    }
}

fn change_interval(data: Arc<Mutex<State>>) {
    loop {
        let time = { //Go out of scope to unlock again
            let mut data = data.lock().unwrap();
            data.next();
            data.change_interval.clone()
        };
        sleep(time);
    }
}
