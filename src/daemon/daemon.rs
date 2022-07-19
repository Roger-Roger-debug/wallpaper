use std::fs::{self, File};
use std::io::{Read, Write};
use std::os::unix::net::*;
use std::os::unix::prelude::{FromRawFd, RawFd};
use std::path::PathBuf;
use std::process::exit;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use clap::{Args, Parser, Subcommand};
use log::info;

use common;

mod state;

use state::*;

//TODO: error handling

/// Struct to hold and parse cli arguments
#[derive(Parser, Debug)]
#[clap(version)]
pub struct Cli {
    /// Image to show by default
    #[clap(short, long, value_parser, value_name = "FILE")]
    default_image: PathBuf,
    /// Socket for communication
    #[clap(short, long, value_parser, value_name = "FILE")]
    socket: Option<PathBuf>,
    /// Directory to search for images, defaults to $HOME/Pictures/backgrounds
    #[clap(short, long, value_parser, value_name = "DIRECTORY")]
    image_directory: Option<PathBuf>,
    /// Time in seconds between wallpaper changes
    #[clap(long, parse(try_from_str = parse_duration))]
    interval: Option<Duration>,
    /// File descriptor to write to to signal readiness
    #[clap(long, default_value_t = 1)]
    fd: RawFd,
    /// Maximum size of the history (used for getting the previous wallpaper)
    #[clap(long, default_value_t = 25)]
    history_length: usize,
    /// Which underlying program to call to change the wallpaper
    #[clap(subcommand)]
    pub method: WallpaperMethod,
}

/// Program that gets called to change the active wallpaper
#[derive(Subcommand, Debug)]
pub enum WallpaperMethod {
    /// Use Feh (for xorg)
    Feh,
    /// Use hyprpaper (for Hyprland / wlroots based wayland compositors)
    Hyprpaper(HyprpaperOptions),
}

/// Hyprpaper needs a list of monitors. This struct holds them
#[derive(Args, Debug)]
pub struct HyprpaperOptions {
    #[clap(value_parser)]
    list: Vec<String>,
}

fn parse_duration(arg: &str) -> Result<std::time::Duration, std::num::ParseIntError> {
    let seconds = arg.parse()?;
    Ok(std::time::Duration::from_secs(seconds))
}

fn main() {
    pretty_env_logger::init();

    let cli = Cli::parse();
    info!("Command run was:\n{:?}", &cli);
    let socket = cli.socket.unwrap_or_else(|| {
        if let Ok(path) = std::env::var("XDG_RUNTIME_DIR") {
            let mut pathbuf = PathBuf::new();
            pathbuf.push(path);
            pathbuf.push("wallpaperd");
            pathbuf
        } else {
            PathBuf::from_str("/tmp/wallpaperd").unwrap()
        }
    });

    let s = socket.clone();
    ctrlc::set_handler(move || {
        if let Err(_) = fs::remove_file(&s) {
            log::error!("Couldn't delete socket file");
        }
        exit(0);
    })
    .expect("Error setting signal hooks");

    let image_dir = cli.image_directory.unwrap_or_else(|| {
        let mut pathbuf = PathBuf::new();
        pathbuf.push(std::env::var("HOME").expect("$HOME not set"));
        pathbuf.push(PathBuf::from("Pictures/backgrounds"));
        pathbuf
    });

    let time = cli.interval.unwrap_or(Duration::new(60, 0));
    let data = Arc::new(Mutex::new(State::new(
        time,
        image_dir,
        cli.default_image,
        cli.method,
        cli.history_length,
    )));

    info!("Binding socket {:?}", socket);
    let listener = UnixListener::bind(&socket).unwrap();
    let mut incoming = listener.incoming();

    let mut file = unsafe { File::from_raw_fd(cli.fd) };
    write!(&mut file, "\n").unwrap();

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

    if let Err(_) = fs::remove_file(&socket) {
        log::error!("Couldn't delete socket file");
    }
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

#[derive(Parser)]
struct ClientMessage {
    #[clap(subcommand)]
    command: common::Command,
}

// Thread: Client <---> Server
fn handle_connection(mut stream: UnixStream, state: Arc<Mutex<State>>) -> bool {
    use common::*;
    use std::io::prelude::*;
    info!("Handle new connection");
    let mut line = read_from_stream(&stream);
    let mut response = "".to_string();

    line = line.to_lowercase();
    info!("Got {}", &line);
    let mut split: Vec<&str> = line.split(" ").collect();
    split.insert(0, " ");
    let mut stop_server = false;
    match ClientMessage::parse_from(split).command {
        Command::Next => state
            .lock()
            .unwrap()
            .change_image(ChangeImageDirection::Next),
        Command::Stop => stop_server = true,
        Command::Previous => state
            .lock()
            .unwrap()
            .change_image(ChangeImageDirection::Previous),
        Command::Mode(mode) => match mode {
            ModeArgs::Linear => state.lock().unwrap().update_image(NextImage::Linear, None),
            ModeArgs::Random => state.lock().unwrap().update_image(NextImage::Random, None),
            ModeArgs::Static(img) => match img.path {
                Some(path) => state
                    .lock()
                    .unwrap()
                    .update_image(NextImage::Static, Some(path)),
                None => state.lock().unwrap().update_image(NextImage::Static, None),
            },
        },
        Command::Fallback => state.lock().unwrap().save(),
        Command::Interval(d) => {
            state.lock().unwrap().change_interval(d.duration);
        }
        Command::Get(what) => {
            response = match what {
                GetArgs::Wallpaper => state
                    .lock()
                    .unwrap()
                    .get_current_image()
                    .clone()
                    .to_str()
                    .unwrap_or("ERROR")
                    .to_owned(),
                GetArgs::Duration => state
                    .lock()
                    .unwrap()
                    .get_change_interval()
                    .as_secs()
                    .to_string(),
                GetArgs::Mode => {
                    let action = state.lock().unwrap().get_action();
                    match action {
                        NextImage::Linear => "Linear".to_string(),
                        NextImage::Static => "Static".to_string(),
                        NextImage::Random => "Random".to_string(),
                    }
                }
                GetArgs::Fallback => state.lock().unwrap().get_fallback().to_string(),
            }
        }
    }

    stream.write(&response.as_bytes()).unwrap();
    stop_server
}

fn change_interval(data: Arc<Mutex<State>>) {
    loop {
        let time = {
            //Go out of scope to unlock again
            let mut unlocked = data.lock().unwrap();
            unlocked.change_image(ChangeImageDirection::Next);
            unlocked.get_change_interval()
        };
        sleep(time);
    }
}
