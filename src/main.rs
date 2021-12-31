mod snapshots;

use std::path::Path;
use std::process;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.get(1) == Some(&"capture".to_string()) {
        if let Some(oid) = snapshots::create(Path::new(".")).unwrap() {
            println!("{}", oid);
        }
    } else {
        dbg!(args);
        eprintln!("Usage: duralumin capture");
        process::exit(1);
    }
}
