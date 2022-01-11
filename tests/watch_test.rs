mod util;

use std::collections::HashSet;
use crate::util::dura::Dura;
use crate::util::git_repo::GitRepo;

#[test]
fn watch_repo() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();

    let dura = Dura::new();
    dura.run_in_dir(&["watch"], tmp.path());

    let mut tmp_set = HashSet::new();
    tmp_set.insert(tmp.path().canonicalize().unwrap());

    assert_eq!(dura.git_repos(), tmp_set);
}

#[test]
fn watch_1_dir_with_2_repos() {
    let tmp = tempfile::tempdir().unwrap();
    let repo1 = GitRepo::new(tmp.path().join("repo1"));
    repo1.init();
    let repo2 = GitRepo::new(tmp.path().join("repo2"));
    repo2.init();

    let dura = Dura::new();
    dura.run_in_dir(&["watch"], tmp.path());

    let mut tmp_set = HashSet::new();
    tmp_set.insert(repo1.dir.canonicalize().unwrap());
    tmp_set.insert(repo2.dir.canonicalize().unwrap());

    assert_eq!(dura.git_repos(), tmp_set);
}

#[test]
fn watch_dir_with_repo_nested_3_folders_deep() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = GitRepo::new(tmp.path().join("a/b/c"));
    repo.init();

    let dura = Dura::new();
    dura.run_in_dir(&["watch"], tmp.path());

    let mut tmp_set = HashSet::new();
    tmp_set.insert(repo.dir.canonicalize().unwrap());

    assert_eq!(dura.git_repos(), tmp_set);
}

