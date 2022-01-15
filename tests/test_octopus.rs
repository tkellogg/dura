use dura::snapshots::CaptureStatus;
use dura::octopus;
use dura::config::RebalanceConfig;
use git2::{Error, Oid};
use crate::util::git_repo::GitRepo;
use crate::util::dura::Dura;

mod util;

#[test]
fn octopus_initial_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 4);

    let cfg = RebalanceConfig::FlatAgg { num_parents: Some(2) };
    let octos = octopus::rebalance(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 2);

    dbg!(&branches);
    assert_eq!(branches[3].commit_hash, get_child(&repo, octos[0], 0).unwrap().to_string());
    assert_eq!(branches[2].commit_hash, get_child(&repo, octos[0], 1).unwrap().to_string());
    assert_eq!(branches[1].commit_hash, get_child(&repo, octos[1], 0).unwrap().to_string());
    assert_eq!(branches[0].commit_hash, get_child(&repo, octos[1], 1).unwrap().to_string());
}

fn create_n_branches(repo: &mut GitRepo, dura: &mut Dura, n: u8) -> Vec<CaptureStatus> {
    repo.write_file("foo.txt");
    let mut ret = Vec::new();

    for _ in 0..n {
        repo.commit_all();
        repo.change_file("foo.txt");
        let commit = dura.capture(repo.dir.as_path()).unwrap();
        ret.push(commit);
    }

    ret
}

fn get_child(repo: &GitRepo, oid: Oid, parent: usize) -> Result<Oid, Error> {
    let git = repo.repo();
    let parents: Vec<_> = git.find_commit(oid)?.parents().collect();
    Ok(parents[parent].id())
}
