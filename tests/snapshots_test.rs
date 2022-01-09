use std::collections::HashMap;
use std::{thread, time};
use dura::config::{Config, WatchConfig};
use dura::snapshots;
use crate::util::GitRepo;

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

    let status = snapshots::capture(repo.dir.path()).unwrap();

    assert_eq!(status, None);
}

/// It keeps capturing commits during a merge conflict
#[test]
fn during_merge_conflicts() {
    let mut repo = util::GitRepo::new();
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
    repo.git(&["status"]);  // debug info

    // change a file anyway
    repo.change_file("foo.txt");
    let status = snapshots::capture(repo.dir.path()).unwrap().unwrap();

    // Regular dura commit
    assert_ne!(status.commit_hash, status.base_hash);
    assert_eq!(status.dura_branch, format!("dura-{}", status.base_hash));
}

#[test]
fn change_single_file_in_multiple_repos() {
    let parent_dir = tempfile::tempdir().unwrap();
    let child_dir = tempfile::tempdir_in(parent_dir.path()).unwrap();

    let mut repos = (0..3)
        .map(|_| GitRepo::new_in(&parent_dir))
        .collect::<Vec<_>>();

    let mut child_repos = (0..3)
        .map(|_| GitRepo::new_in(&child_dir))
        .collect::<Vec<_>>();

    let mut repo_map = HashMap::new();
    repo_map.insert(
        parent_dir.path().as_os_str().to_str().unwrap().to_string(),
        WatchConfig {
            exclude: vec![
                repos[0].dir.path().to_str().unwrap().to_string(),
                child_dir.path().as_os_str().to_str().unwrap().to_string(),
            ],
            include: vec![child_repos[0].dir.path().to_str().unwrap().to_string()],
            max_depth: 255
        },
    );

    let config = Config {
        pid: None,
        repos: repo_map,
    };

    let mut dura = util::Dura::new();

    config.save_to_path(&dura.config_path());

    assert_ne!(None, dura.get_config());

    repos.append(&mut child_repos);
    for mut repo in repos {
        repo.write_file("foo.txt");
        repo.commit_all();

        repo.change_file("foo.txt");

        // FIXME: cannot find repo
        let status = snapshots::capture(repo.dir.path()).unwrap().unwrap();

        println!("{:?}", status);
    }

    assert_ne!(None, dura.pid(true));
    let cfg = dura.get_config();
    assert_ne!(None, cfg);
    assert_eq!(dura.pid(true), cfg.unwrap().pid);
}
