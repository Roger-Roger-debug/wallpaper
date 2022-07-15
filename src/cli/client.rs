use clap::Parser;
use std::io::prelude::*;
use std::os::unix::net::UnixStream;
use std::path::PathBuf;
use std::str::FromStr;

use log::info;

#[derive(Parser)]
#[clap(version)]
pub struct Cli {
    /// Socket for communication
    #[clap(short, long, value_parser, value_name = "FILE")]
    socket: Option<PathBuf>,
    #[clap(subcommand)]
    command: common::Command,
}

fn main() {
    pretty_env_logger::init();

    let cli = Cli::parse();
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

    let mut socket = UnixStream::connect(socket).expect("Socket not found");

    //TODO make not shit
    let args = cli.command.to_string();

    let len = args.len();

    info!("Sending {:?}", args);
    {
        socket.write(&len.to_ne_bytes()).unwrap();
        socket.write(args.trim().as_bytes()).unwrap();
        info!("{:?}", &len.to_ne_bytes());
        info!("{:?}", args.trim().as_bytes());
        socket.flush().unwrap();
    }

    info!("Reading:");
    let mut line = String::new();
    socket
        .read_to_string(&mut line)
        .expect("Couldn't read string");
    println!("{}", line);
}
