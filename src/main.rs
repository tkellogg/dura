mod snapshots;

use std::path::Path;

fn main() {
    if let Some(oid) = snapshots::create(Path::new(".")).unwrap() {
        println!("{}", oid);
    }
}
