use std::path::Path;
use std::process;
use std::io::stdout;
use std::io::Write;

use tokio::time;

use crate::snapshots;
use crate::config::Config;

fn process_directory(path: &Path) {
    if let Some(oid) = snapshots::capture(path).unwrap() {
        println!("{}", oid);
    } else {
        print!(".");
    }
    stdout().flush().unwrap();
}

fn do_task() {
    let config = Config::load();
    if config.pid != Some(process::id()) {
        eprintln!("Shutting down because other poller took lock: {:?}", config.pid);
        process::exit(1);
    }

    for (key, _value) in config.repos {
        let path = Path::new(key.as_str());
        process_directory(&path);
    }
}

pub async fn start() {
    let mut config = Config::load();
    config.pid = Some(process::id());
    config.save();

    loop {
        time::sleep(time::Duration::from_secs(5)).await;
        do_task();
    }
}
