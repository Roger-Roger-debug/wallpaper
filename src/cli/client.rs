use clap::{app_from_crate, arg, App, Arg};
use std::env;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::str::FromStr;

use log::info;

fn main() {
    pretty_env_logger::init();

    let matches = app_from_crate!()
        .arg(arg!(-s - -socket[PATH]))
        .subcommand(App::new("next").about("Show next image"))
        .subcommand(App::new("stop").about("Stop the daemon"))
        .subcommand(App::new("prev").about("Show the previous image"))
        .subcommand(
            App::new("mode")
                .about("Change the image order")
                .subcommand(App::new("linear").visible_alias("lin"))
                .subcommand(App::new("random").visible_alias("rng"))
                .subcommand(App::new("static").arg(Arg::new("[PATH]").required(false))),
        )
        .subcommand(
            App::new("fallback")
                .about("Display the fallback image")
                .alias("save"),
        )
        .subcommand(App::new("update").about("Update the Image cache"))
        .subcommand(App::new("shuffle").about("Shuffle image order"))
        .subcommand(
            App::new("interval")
                .arg(arg!(<SECONDS>))
                .about("Set image change interval"),
        )
        .subcommand(
            App::new("get")
                .about("Get information about the current settings")
                .subcommand(App::new("wallpaper").visible_alias("wp"))
                .subcommand(App::new("duration").visible_alias("dur"))
                .subcommand(App::new("mode").visible_alias("m")),
        )
        .get_matches();

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
    let mut socket = UnixStream::connect(socket_path).expect("Socket not found");

    let args = match matches.subcommand() {
        Some(("next", _)) => "next".to_owned(),
        Some(("prev", _)) => "prev".to_owned(),
        Some(("stop", _)) => "stop".to_owned(),
        Some(("update", _)) => "update".to_owned(),
        Some(("shuffle", _)) => "shuffle".to_owned(),
        Some(("fallback", _)) => "save".to_owned(),
        Some(("mode", sub_matches)) => match sub_matches.subcommand() {
            Some(("random", _)) => "rng".to_owned(),
            Some(("linear", _)) => "linear".to_owned(),
            Some(("static", sub_matches)) => match sub_matches.value_of("PATH") {
                Some(path) => {
                    format!("hold {}", path)
                }
                None => "hold".to_owned(),
            },
            Some((_, _)) => "help".to_owned(),
            None => "help".to_owned(),
        },
        Some(("get", sub_matches)) => match sub_matches.subcommand() {
            Some(("wallpaper", _)) => "get wp".to_owned(),
            Some(("duration", _)) => "get duration".to_owned(),
            Some(("mode", _)) => "get action".to_owned(),
            Some((_, _)) => "help".to_owned(),
            None => "help".to_owned(),
        },
        Some(("interval", sub_matches)) => {
            let msg = format!(
                "interval {}",
                sub_matches.value_of("SECONDS").expect("Interval missing")
            );
            msg
        }
        Some((_, _)) => "help".to_owned(),
        None => "help".to_owned(),
    };

    let len = args.len();

    info!("Sending {:?}", args);
    {
        socket.write(&len.to_ne_bytes()).unwrap();
        info!("{:?}", &len.to_ne_bytes());
        info!("{:?}", args.trim().as_bytes());
        socket.write(args.trim().as_bytes()).unwrap();
        socket.flush().unwrap();
    }

    info!("Reading:");
    let mut line = String::new();
    socket
        .read_to_string(&mut line)
        .expect("Couldn't read string");
    println!("{}", line);
}
