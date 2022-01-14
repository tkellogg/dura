use std::{thread, time};
use dura::snapshots;
use dura::snapshots::CaptureStatus;
use dura::octopus;
use dura::config::RebalanceConfig;
use git2::{Error, Oid};
use crate::util::git_repo::GitRepo;

mod util;

#[test]
fn octopus_initial_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let branches = create_n_branches(&mut repo, 4);

    let cfg = RebalanceConfig::FlatAgg { num_parents: Some(2) };
    let octos = octopus::rebalance(tmp.path(), &cfg).unwrap();

    dbg!(&branches);
    dbg!(&branches.iter().map(|b| repo.repo().find_commit(Oid::from_str(b.commit_hash.as_str()).unwrap()).unwrap().time()).collect::<Vec<_>>());
    assert_eq!(branches[3].commit_hash, get_child(&repo, octos[0], 0).unwrap().to_string());
    assert_eq!(branches[2].commit_hash, get_child(&repo, octos[0], 1).unwrap().to_string());
    assert_eq!(branches[1].commit_hash, get_child(&repo, octos[1], 0).unwrap().to_string());
    assert_eq!(branches[0].commit_hash, get_child(&repo, octos[1], 1).unwrap().to_string());
}

fn create_n_branches(repo: &mut GitRepo, n: u8) -> Vec<CaptureStatus> {
    repo.write_file("foo.txt");
    let mut ret = Vec::new();

    for _ in 0..n {
        repo.commit_all();
        repo.change_file("foo.txt");
        let commit = snapshots::capture(repo.dir.as_path()).unwrap().unwrap();
        ret.push(commit);
        thread::sleep(time::Duration::from_millis(500));
    }

    ret
}

fn get_child(repo: &GitRepo, oid: Oid, parent: usize) -> Result<Oid, Error> {
    let git = repo.repo();
    let parents: Vec<_> = git.find_commit(oid)?.parents().collect();
    Ok(parents[parent].id())
}
