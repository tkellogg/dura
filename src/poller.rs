use std::path::Path;
use std::io::stdout;
use std::io::Write;

use tokio::time;

use crate::snapshots;

fn do_task() {
    if let Some(oid) = snapshots::capture(Path::new(".")).unwrap() {
        println!("{}", oid);
    } else {
        print!(".");
    }
    stdout().flush().unwrap();
}

pub async fn start() {
    loop {
        time::sleep(time::Duration::from_secs(5)).await;
        do_task();
    }
}
