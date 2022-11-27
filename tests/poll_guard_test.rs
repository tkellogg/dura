use dura::poll_guard::PollGuard;
use dura::snapshots;
use std::thread::sleep;
use std::time::Duration;

mod util;

#[test]
fn changed_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");
    let mut pg = PollGuard::new();
    assert!(!pg.dir_changed(repo.dir.as_path()));

    sleep(Duration::from_secs_f64(1.5));
    repo.change_file("foo.txt");
    assert!(pg.dir_changed(repo.dir.as_path()));
}

/// Changing a branch shouldn't trigger the slow process
#[test]
fn branch_changed() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_and_file!(tmp, "foo.txt");
    let mut pg = PollGuard::new();
    assert!(!pg.dir_changed(repo.dir.as_path()));

    sleep(Duration::from_secs_f64(1.5));
    repo.git(&["checkout", "-b", "new-branch"])
        .expect("checkout failed");
    assert!(!pg.dir_changed(repo.dir.as_path()));
}

#[test]
fn file_changed_after_snapshot() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");
    let mut pg = PollGuard::new();
    assert!(!pg.dir_changed(repo.dir.as_path()));

    sleep(Duration::from_secs_f64(1.5));
    repo.change_file("foo.txt");
    assert!(pg.dir_changed(repo.dir.as_path()));

    sleep(Duration::from_secs_f64(1.5));
    snapshots::capture(repo.dir.as_path()).expect("snapshot failed");
    assert!(!pg.dir_changed(repo.dir.as_path()));

    sleep(Duration::from_secs_f64(1.5));
    repo.change_file("foo.txt");
    assert!(pg.dir_changed(repo.dir.as_path()));
}
