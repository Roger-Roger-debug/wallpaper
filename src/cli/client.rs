use std::env;
use std::os::unix::net::UnixStream;
use std::io::prelude::*;

use log::info;

fn main() {
    pretty_env_logger::init();
    let mut socket = UnixStream::connect("/tmp/test.socket").expect("Socket not found");

    let args = env::args().skip(1).map(|arg| arg + " ").collect::<String>();
    let args = args.trim();
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
    socket.read_to_string(&mut line).expect("Couldn't read string");
    println!("{}", line);
}
