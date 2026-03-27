use dura::{config::Config, snapshots};

use std::env;

mod util;

#[macro_use]
extern crate serial_test;

#[test]
fn change_single_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.dura_branch, format!("dura/{}", status.base_hash));
    assert_eq!(status.dura_branch, format!("dura/{}", status.base_hash));
}

#[test]
fn no_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = repo_and_file!(tmp, "foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap();

    assert_eq!(status, None);
}

/// It keeps capturing commits during a merge conflict
#[test]
fn during_merge_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");

    // branch1
    repo.change_file("foo.txt");
    repo.commit_all();
    repo.git(&["checkout", "-b", "branch1"]).unwrap();

    // branch2
    repo.git(&["checkout", "-b", "branch2"]).unwrap();
    repo.git(&["reset", "HEAD^", "--hard"]).unwrap();
    repo.change_file("foo.txt");
    repo.commit_all();

    // MERGE FAIL
    let merge_result = repo.git(&["merge", "branch1"]);
    assert_eq!(merge_result, None);
    repo.git(&["status"]).unwrap(); // debug info

    // change a file anyway
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    // Regular dura commit
    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.dura_branch, format!("dura/{}", status.base_hash));
}

#[test]
#[serial]
fn test_commit_signature_using_dura_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("DURA_CONFIG_HOME", tmp.path());
    let mut dura_config = Config::empty();
    dura_config.commit_author = Some("dura-config".to_string());
    dura_config.commit_email = Some("dura-config@email.com".to_string());
    dura_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo.git(&["show", "-s", "--format=format:%an", &status.commit_hash]);
    assert_eq!(commit_author, dura_config.commit_author);

    let commit_email = repo.git(&["show", "-s", "--format=format:%ae", &status.commit_hash]);
    assert_eq!(commit_email, dura_config.commit_email);
}

#[test]
#[serial]
fn test_commit_signature_using_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("DURA_CONFIG_HOME", tmp.path());
    let dura_config = Config::empty();
    dura_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo
        .git(&["show", "-s", "--format=format:%an", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_author, "git-author");

    let commit_email = repo
        .git(&["show", "-s", "--format=format:%ae", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_email, "git@someemail.com");
}

#[test]
#[serial]
fn test_commit_signature_exclude_git_config() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.set_config("user.name", "git-author");
    repo.set_config("user.email", "git@someemail.com");

    env::set_var("DURA_CONFIG_HOME", tmp.path());
    let mut dura_config = Config::empty();
    dura_config.commit_exclude_git_config = true;
    dura_config.save();

    repo.write_file("foo.txt");
    repo.commit_all();
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let commit_author = repo
        .git(&["show", "-s", "--format=format:%an", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_author, "dura");

    let commit_email = repo
        .git(&["show", "-s", "--format=format:%ae", &status.commit_hash])
        .unwrap();
    assert_eq!(commit_email, "dura@github.io");
}

/// Commit messages should include file names and change stats
#[test]
fn commit_message_includes_file_info() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "model.py");
    repo.change_file("model.py");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let message = repo
        .git(&["show", "-s", "--format=format:%s", &status.commit_hash])
        .unwrap();
    assert!(message.starts_with("dura: 1 file"), "got: {}", message);
    assert!(message.contains("model.py"), "got: {}", message);
}

/// Commit messages with multiple files show stats and file names
#[test]
fn commit_message_multiple_files() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = repo_and_file!(tmp, "foo.txt");
    repo.write_file("bar.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    repo.change_file("bar.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    let message = repo
        .git(&["show", "-s", "--format=format:%s", &status.commit_hash])
        .unwrap();
    assert!(message.starts_with("dura: 2 files"), "got: {}", message);
    assert!(message.contains("foo.txt"), "got: {}", message);
    assert!(message.contains("bar.txt"), "got: {}", message);
}

/// Repos with no commits (unborn branch) should be captured without error
#[test]
fn unborn_branch_first_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    // Don't commit — this is the unborn branch case
    repo.write_file("foo.txt");

    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    assert_eq!(status.dura_branch, "dura/unborn");
    assert_eq!(status.base_hash, "unborn");
}

/// Subsequent captures on an unborn branch reuse the prior dura commit as parent
#[test]
fn unborn_branch_subsequent_capture() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");

    // First capture
    let status1 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();
    assert_eq!(status1.dura_branch, "dura/unborn");

    // Change the file and capture again
    repo.change_file("foo.txt");
    let status2 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    assert_eq!(status2.dura_branch, "dura/unborn");
    assert_ne!(status2.commit_hash, status1.commit_hash);
}

/// After user makes their first commit, dura switches from unborn to normal branch
#[test]
fn unborn_branch_transitions_to_normal() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");

    // Capture on unborn
    let status1 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();
    assert_eq!(status1.dura_branch, "dura/unborn");

    // User makes their first commit
    repo.commit_all();
    // Change a file
    repo.change_file("foo.txt");

    // Now dura should use the normal branch naming
    let status2 = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();
    assert!(status2.dura_branch.starts_with("dura/"));
    assert_ne!(status2.dura_branch, "dura/unborn");
    assert_ne!(status2.base_hash, "unborn");
}
