use std::env;
use dura::snapshots;

mod util;

#[test]
fn change_single_file() {
    let mut repo = util::GitRepo::new();
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.path()).unwrap().unwrap();

    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.dura_branch, format!("dura-{}", status.base_hash));
}

#[test]
fn no_changes() {
    let repo = util::GitRepo::new();
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    println!("$ pwd");
    println!("{:?}", env::current_dir().unwrap());
    println!("$ dura capture {}", repo.dir.path().to_str().unwrap());
    let status = snapshots::capture(repo.dir.path()).unwrap();

    assert_eq!(status, None);
}

