use std::path::Path;
use git2::{Error, Repository, BranchType, Branch, Time, Commit, Oid};

use crate::config::RebalanceConfig;

/// Create or rebalance the octo-tree cold history.
pub fn rebalance(repo_path: &Path, config: &RebalanceConfig) -> Result<Vec<Oid>, Error> {
    let repo = Repository::open(repo_path)?;
    let hash_branches = get_hash_branches(&repo)?;
    let parent_commits: Vec<_> = hash_branches.iter()
        .flat_map(|branch| branch.get().peel_to_commit().ok())
        .collect();
    let parents = to_refs(&parent_commits);

    match config {
        RebalanceConfig::FlatAgg{ num_parents, num_uncompressed } => {
            match get_args(*num_parents, *num_uncompressed, parents) {
                Some((num_parents, commits)) => {
                    Ok(build_tree(&repo, commits, num_parents)?)
                }
                None => Ok(vec![])
            }
        }
        RebalanceConfig::Tree{ num_parents, num_uncompressed } => {
            match get_args(*num_parents, *num_uncompressed, parents) {
                Some((num_parents, commits)) => {
                    let mut last_pass: Vec<Commit> = commits.iter().map(|x| *x.clone()).collect();
                    loop {
                        // parents[0] is the newest
                        let last_pass_oids = build_tree(&repo, to_refs(&last_pass), num_parents)?;
                        if last_pass_oids.len() > 0 {
                            last_pass = last_pass_oids.iter()
                                .flat_map(|oid| repo.find_commit(*oid).ok())
                                .collect();
                        } else {
                            return Ok(last_pass_oids);
                        }
                    }
                }
                None => Ok(vec![])
            }
        }
    }
}

fn get_args<'a, T>(num_parents: Option<u8>, num_uncompressed: Option<u16>, parents: &'a [&'a T]) -> Option<(u8, &'a [&'a T])> {
    if let Some(num_uncompressed) = num_uncompressed {
        if (num_uncompressed as usize) < parents.len() {
            // parents[0] is the newest
            Some((num_parents.unwrap_or(8), (&parents[(num_uncompressed as usize)..])))
        } else {
            None
        }
    } else {
        // Setting num_uncompressed to None/null means we don't compress any branches.
        None
    }
}

/// I couldn't find this in std:: probably because the lifetime makes it awkward to use
fn to_refs<'a, T>(vec: &'a Vec<T>) -> &'a [&'a T] {
    let refs: Vec<_> = vec.iter()
        .map(|item| item)
        .collect();
    &refs[..]
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

/// Build a single layer of a tree. We're still not sure what we want out of a branch compaction
/// routine, so this is flexible enough to serve 2 use cases — a smaller amount of flat
/// "octopuses" (merge commits with >2 parents) or a hierarchical "B-tree" (merge commits
/// recursively rolling up into a single branch of cold branches).
fn build_tree<'a>(repo: &'a Repository, parent_commits: &[&'a Commit], num_parents: u8) -> Result<Vec<Oid>, Error> {
    let mut ret: Vec<Oid> = Vec::new();

    // parents[0] is the newest
    for parents in parent_commits.chunks(num_parents.into()) {
        if parents.len() == 0 {
            break;
        }

        let message = "dura compacted commit";

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

