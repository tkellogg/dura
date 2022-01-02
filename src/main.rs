mod config;
mod snapshots;
mod poller;

use std::path::Path;
use std::process;
use crate::config::{Config, WatchConfig};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1) == Some(&"capture".to_string()) {
        if let Some(oid) = snapshots::capture(Path::new(".")).unwrap() {
            println!("{}", oid);
        }
    } else if args.get(1) == Some(&"serve".to_string()) {
        println!("pid: {}", std::process::id());
        poller::start().await;
    } else if args.get(1) == Some(&"watch".to_string()) {
        watch_dir(".");
    } else if args.get(1) == Some(&"kill".to_string()) {
        kill();
    } else {
        dbg!(args);
        eprintln!("Usage: dura capture");
        process::exit(1);
    }
}

fn watch_dir(dir: &str) {
    let path = Path::new(dir).canonicalize().unwrap();

    let mut config = Config::load();
    config.set_watch(path.as_path().to_str().unwrap().to_string(), WatchConfig::new());
    config.save();
}

fn kill() {
    let mut config = Config::load();
    config.pid = None;
    config.save();
}

