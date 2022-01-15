use std::path::Path;
use git2::{Error, Repository, BranchType, Branch, Time, Commit, Oid};

use crate::config::RebalanceConfig;

/// Create or rebalance the octo-tree cold history.
pub fn rebalance(repo_path: &Path, config: &RebalanceConfig) -> Result<Vec<Oid>, Error> {
    let repo = Repository::open(repo_path)?;
    let hash_branches = get_hash_branches(&repo)?;
    match config {
        RebalanceConfig::FlatAgg{ num_parents } => {
            let parent_commits: Vec<_> = hash_branches.iter()
                .flat_map(|branch| branch.get().peel_to_commit().ok())
                .collect();
            let parents: Vec<_> = parent_commits.iter()
                .map(|commit| commit)
                .collect();

            Ok(build_tree(&repo, &parents[..], num_parents.unwrap_or(8))?)
        }
    }
}

fn get_hash_branches(repo: &Repository) -> Result<Vec<Branch>, Error> {
    let mut ret: Vec<_> = repo.branches(Some(BranchType::Local))?
        .flat_map(|res| res.into_iter())
        .map(|tuple| {
            let (branch, _) = tuple;
            branch
        })
        .filter(|branch| match branch.name() {
            Ok(Some(name)) => name.starts_with("dura/") && name.split("/").count() == 2,
            _ => false
        })
        .collect();

    //dbg!(ret.iter().map(|b| b.get().peel_to_commit().map(|c| c.author().when())).collect::<Vec<_>>());
    sort_desc(&mut ret);

    Ok(ret)
}

fn sort_desc<'repo>(branches: &mut Vec<Branch<'repo>>) {
    branches.sort_by(|a, b| {
        let a_time = a.get().peel_to_commit()
            .map(|c| c.time())
            .unwrap_or(Time::new(0, 0));
        let b_time = b.get().peel_to_commit()
            .map(|c| c.time())
            .unwrap_or(Time::new(0, 0));

        b_time.cmp(&a_time)
    });
}

fn build_tree<'a>(repo: &'a Repository, parent_commits: &[&'a Commit], num_parents: u8) -> Result<Vec<Oid>, Error> {
    let mut ret: Vec<Oid> = Vec::new();

    let mut num_pages = parent_commits.len() / (num_parents as usize);
    if (parent_commits.len() % (num_parents as usize)) != 0 {
        // when pages aren't perfectly aligned, add an extra page
        num_pages += 1;
    }

    for page in 0..num_pages {
        let parents = &parent_commits[(page*(num_parents as usize))..((page+1)*(num_parents as usize))];
        if parents.len() == 0 {
            break;
        }

        let message = "dura summary tree";

        let oid = repo.commit(
            None,
            &parents[0].author(),
            &parents[0].committer(),
            message,
            &parents[0].tree()?,
            &parents[..],
        )?;

        ret.push(oid);
    }

    Ok(ret)
}

