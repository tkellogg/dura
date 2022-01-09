use dura::snapshots;

mod util;

#[test]
fn change_single_file() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.dura_branch, format!("dura/{}", status.base_hash));
}

#[test]
fn no_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    repo.write_file("foo.txt");
    repo.commit_all();

    let status = snapshots::capture(repo.dir.as_path()).unwrap();

    assert_eq!(status, None);
}

/// It keeps capturing commits during a merge conflict
#[test]
fn during_merge_conflicts() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = util::git_repo::GitRepo::new(tmp.path().to_path_buf());
    repo.init();

    // parent commit
    repo.write_file("foo.txt");
    repo.commit_all();

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
    repo.git(&["status"]); // debug info

    // change a file anyway
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();

    // Regular dura commit
    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.dura_branch, format!("dura/{}", status.base_hash));
}
