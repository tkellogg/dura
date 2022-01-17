use std::path::Path;
use git2::{Error, Repository, BranchType, Branch, Time, Commit, Oid};

use crate::config::RebalanceConfig;

/// Create or rebalance the octo-tree cold history.
pub fn rebalance(repo_path: &Path, config: &RebalanceConfig) -> Result<Vec<Oid>, Error> {
    let repo = Repository::open(repo_path)?;
    let hash_branches = get_hash_branches(&repo)?;
    match config {
        RebalanceConfig::FlatAgg{ num_parents, num_uncompressed } => {
            let parent_commits: Vec<_> = hash_branches.iter()
                .flat_map(|branch| branch.get().peel_to_commit().ok())
                .collect();
            let parents: Vec<_> = parent_commits.iter()
                .map(|commit| commit)
                .collect();

            if let Some(num_uncompressed) = num_uncompressed {
                if (*num_uncompressed as usize) < parents.len() {
                    Ok(build_tree(&repo, &parents[(*num_uncompressed as usize)..], num_parents.unwrap_or(8))?)
                } else {
                    Ok(vec![])
                }
            } else {
                // Setting num_uncompressed to None/null means we don't compress any branches.
                Ok(vec![])
            }
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

    sort(&mut ret);

    Ok(ret)
}

fn sort<'repo>(branches: &mut Vec<Branch<'repo>>) {
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

    for parents in parent_commits.chunks(num_parents.into()) {
        if parents.len() == 0 {
            break;
        }

        let message = "dura compacted commit";
        //let reversed = parents.iter().rev().map(|x| *x).collect::<Vec<&Commit>>();
        //let parents = &reversed[..];

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

