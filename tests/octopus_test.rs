use crate::util::dura::Dura;
use crate::util::git_repo::GitRepo;
use dura::config::ConsolidateStrategy;
use dura::octopus;
use dura::snapshots::CaptureStatus;
use git2::{Error, Oid};

mod util;

///    *         *
///  /   \     /   \
/// *      *  *     *
#[test]
fn octopus_initial_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 4);

    let cfg = ConsolidateStrategy::Flat {
        num_parents: Some(2),
        num_uncompressed: Some(0),
    };
    let octos = octopus::consolidate(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 2);

    // branches[0] is the oldest
    assert_eq!(
        branches[0].commit_hash,
        get_child(&repo, octos[1], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[1].commit_hash,
        get_child(&repo, octos[1], 0).unwrap().to_string()
    );
    assert_eq!(
        branches[2].commit_hash,
        get_child(&repo, octos[0], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[3].commit_hash,
        get_child(&repo, octos[0], 0).unwrap().to_string()
    );

    let git = repo.repo();
    let tags = octopus::get_flat_tags(&git).unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(
        git.find_reference("refs/tags/dura/cold/1")
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id(),
        octos[1],
    );
    assert_eq!(
        git.find_reference("refs/tags/dura/cold/2")
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id(),
        octos[0],
    );
}

/// When num_uncompressed == 1, an extra commit is not added to the tree
///
///      *        *
///    /   \    /   \
/// * *     *  *     *
#[test]
fn num_uncompressed_eq_1() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 5);

    let cfg = ConsolidateStrategy::Flat {
        num_parents: Some(2),
        num_uncompressed: Some(1),
    };
    let octos = octopus::consolidate(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 2);

    // branches[0] is the oldest
    assert_eq!(
        branches[0].commit_hash,
        get_child(&repo, octos[1], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[1].commit_hash,
        get_child(&repo, octos[1], 0).unwrap().to_string()
    );
    assert_eq!(
        branches[2].commit_hash,
        get_child(&repo, octos[0], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[3].commit_hash,
        get_child(&repo, octos[0], 0).unwrap().to_string()
    );

    let git = repo.repo();
    let tags = octopus::get_flat_tags(&git).unwrap();
    assert_eq!(tags.len(), 2);
    assert_eq!(
        git.find_reference("refs/tags/dura/cold/1")
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id(),
        octos[1],
    );
    assert_eq!(
        git.find_reference("refs/tags/dura/cold/2")
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id(),
        octos[0],
    );
}

/// When num_uncompressed == 0, the extra commit is added to an octopus
///
/// *    *        *
/// |  /   \    /   \
/// * *     *  *     *
#[test]
fn num_uncompressed_eq_0() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 5);

    let cfg = ConsolidateStrategy::Flat {
        num_parents: Some(2),
        num_uncompressed: Some(0),
    };
    let octos = octopus::consolidate(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 3);

    // branches[0] is the oldest
    assert_eq!(
        branches[0].commit_hash,
        get_child(&repo, octos[2], 0).unwrap().to_string()
    );
    assert_eq!(
        branches[1].commit_hash,
        get_child(&repo, octos[1], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[2].commit_hash,
        get_child(&repo, octos[1], 0).unwrap().to_string()
    );
    assert_eq!(
        branches[3].commit_hash,
        get_child(&repo, octos[0], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[4].commit_hash,
        get_child(&repo, octos[0], 0).unwrap().to_string()
    );
}

/// When num_uncompressed == 3, the extra commit is added to an octopus
///
///       *     *
///       |   /   \
/// *  *  *  *     *
#[test]
fn num_uncompressed_eq_2() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 5);

    let cfg = ConsolidateStrategy::Flat {
        num_parents: Some(2),
        num_uncompressed: Some(2),
    };
    let octos = octopus::consolidate(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 2);

    // branches[0] is the oldest
    assert_eq!(
        branches[0].commit_hash,
        get_child(&repo, octos[1], 0).unwrap().to_string()
    );
    assert_eq!(
        branches[1].commit_hash,
        get_child(&repo, octos[0], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[2].commit_hash,
        get_child(&repo, octos[0], 0).unwrap().to_string()
    );
}

/// Very wide Flat strategy
///
///
///       *
///   ___/|\___   
///  / /  |  \ \      
/// *  *  *   * *
#[test]
fn num_parents_eq_5_num_uncompressed_eq_0() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 5);

    let cfg = ConsolidateStrategy::Flat {
        num_parents: Some(5),
        num_uncompressed: Some(0),
    };
    let octos = octopus::consolidate(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 1);

    // branches[0] is the oldest
    assert_eq!(
        branches[0].commit_hash,
        get_child(&repo, octos[0], 4).unwrap().to_string()
    );
    assert_eq!(
        branches[1].commit_hash,
        get_child(&repo, octos[0], 3).unwrap().to_string()
    );
    assert_eq!(
        branches[2].commit_hash,
        get_child(&repo, octos[0], 2).unwrap().to_string()
    );
    assert_eq!(
        branches[3].commit_hash,
        get_child(&repo, octos[0], 1).unwrap().to_string()
    );
    assert_eq!(
        branches[4].commit_hash,
        get_child(&repo, octos[0], 0).unwrap().to_string()
    );
}

/// Tree strategy
///       *
///     /  \
///    /    \
///   *      *    
///  / \    / \           
/// *   *  *   *
#[test]
fn tree_2_levels() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches = create_n_branches(&mut repo, &mut dura, 4);

    let cfg = ConsolidateStrategy::Tree {
        num_parents: Some(2),
        num_uncompressed: Some(0),
    };
    let octos = octopus::consolidate(tmp.path(), &cfg).unwrap();
    assert_eq!(octos.len(), 1);

    // branches[0] is the oldest
    assert_eq!(
        branches[0].commit_hash,
        get_child_2(&repo, octos[0], 1, 1).unwrap().to_string()
    );
    assert_eq!(
        branches[1].commit_hash,
        get_child_2(&repo, octos[0], 1, 0).unwrap().to_string()
    );
    assert_eq!(
        branches[2].commit_hash,
        get_child_2(&repo, octos[0], 0, 1).unwrap().to_string()
    );
    assert_eq!(
        branches[3].commit_hash,
        get_child_2(&repo, octos[0], 0, 0).unwrap().to_string()
    );

    let git = repo.repo();
    assert_eq!(
        git.find_reference("refs/tags/dura/cold")
            .unwrap()
            .peel_to_commit()
            .unwrap()
            .id(),
        octos[0],
    );
}

/// Secondary compact – add a branch to complete the tree
///    *         *
///      \     /   \
/// +      *  *     *
#[test]
fn flat_secondary_compact() {
    let tmp = tempfile::tempdir().unwrap();
    let mut repo = GitRepo::new(tmp.path().to_path_buf());
    repo.init();
    let mut dura = Dura::new();
    let branches_1 = create_n_branches(&mut repo, &mut dura, 3);

    let cfg = ConsolidateStrategy::Flat {
        num_parents: Some(2),
        num_uncompressed: Some(0),
    };
    octopus::consolidate(tmp.path(), &cfg).unwrap();
    let branches_2 = create_n_branches(&mut repo, &mut dura, 1);
    println!("consolidate");
    octopus::consolidate(tmp.path(), &cfg).unwrap();

    let git = repo.repo();
    let tags = octopus::get_flat_tags(&git).unwrap();
    assert_eq!(tags.len(), 2);
    let tags = vec![
        git.find_reference("refs/tags/dura/cold/1")
            .unwrap()
            .peel_to_commit()
            .unwrap(),
        git.find_reference("refs/tags/dura/cold/2")
            .unwrap()
            .peel_to_commit()
            .unwrap(),
    ];

    dbg!(&branches_1, &tags[0].parents().collect::<Vec<_>>(), &tags[1].parents().collect::<Vec<_>>());
    // branches[0] is the oldest
    assert_eq!(
        branches_1[0].commit_hash,
        get_child(&repo, tags[0].id(), 1).unwrap().to_string()
    );
    assert_eq!(
        branches_1[1].commit_hash,
        get_child(&repo, tags[0].id(), 0).unwrap().to_string()
    );
    assert_eq!(
        branches_1[2].commit_hash,
        get_child(&repo, tags[1].id(), 1).unwrap().to_string()
    );
    dbg!(&branches_2[0]);
    assert_eq!(
        branches_2[0].commit_hash,
        get_child(&repo, tags[1].id(), 0).unwrap().to_string()
    );
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

/// Get child 2-layers deep
fn get_child_2(repo: &GitRepo, oid: Oid, parent: usize, parent_2: usize) -> Result<Oid, Error> {
    let git = repo.repo();
    let parents: Vec<_> = git.find_commit(oid)?.parents().collect();
    let parents_2: Vec<_> = git.find_commit(parents[parent].id())?.parents().collect();
    Ok(parents_2[parent_2].id())
}
