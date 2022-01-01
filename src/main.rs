mod snapshots;
mod poller;

use std::path::Path;
use std::process;

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
    } else {
        dbg!(args);
        eprintln!("Usage: duralumin capture");
        process::exit(1);
    }
}
