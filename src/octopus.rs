use git2::{Branch, BranchType, Commit, Error, Oid, Repository, Time};
use std::ops::Deref;
use std::path::Path;

use crate::config::ConsolidateStrategy;

/// Maximum recursion level when running the tree builder algorithm. This limits to the number of
/// branches that can be summarized to 2**n worst case, it's actually num_parents**n. So n==16
/// means at least 65,536 branches can be summarized. This is insanely high, and can be made much
/// higher by increasing num_parents. No one should be running into this limit.
const MAX_TREE_HEIGHT: usize = 16;

/// De-clutters dura branches by combining existing branches into "cold storage" tags. The snapshot
/// branches are combined via "octopus" commits, i.e. merge commits with more than 2 parents. 
///
/// There are 2 main strategies:
/// 
///  1. Flat - snapshot branches are consolidated into far fewer tags.
///  2. Tree - Effectively "single branch mode". Snapshot branches are recursively rolled up into
///     octopus merge commits (a la Flat, but recursively) until there's a single commit on top.
///     This commit is tagged as `dura/cold`.
///
/// Both of these strategies have some options in common:
///
///  * num_uncompressed - The number of snapshot branches to exclude from consolidation. To get
///    true single-branch mode, use Tree with num_uncompressed=0.
///  * num_parents - The number of parent commits that each merge commit should have, or, how many
///    legs should the octopus have? This is technically unlimited, but should probably be kept
///    under 60.
pub fn consolidate(repo_path: &Path, config: &ConsolidateStrategy) -> Result<Vec<Oid>, Error> {
    let repo = Repository::open(repo_path)?;
    let mut hash_branches = get_dura_snapshot_branches(&repo)?;

    // Not sure what order the branches come back in, so let's take control. We need them to be in
    // reverse order, so newest is [0] and we can slice off num_uncompressed easily enough.
    sort(&mut hash_branches);

    let parent_commits: Vec<_> = hash_branches
        .iter()
        .flat_map(|branch| branch.get().peel_to_commit().ok())
        .collect();
    let parents = to_refs(&parent_commits);

    match config {
        // Flat is just the bottom level of Tree. All snapshot branches are combined into
        // "octopus" commits. These become tags and are named "dura/cold/1", "dura/cold/2",...
        ConsolidateStrategy::Flat {
            num_parents,
            num_uncompressed,
        } => match get_args(*num_parents, *num_uncompressed, &parents[..]) {
            Some((num_parents, commits)) => Ok(build_tree(&repo, commits, num_parents)?),
            None => Ok(vec![]),
        },
        // Tree
        ConsolidateStrategy::Tree {
            num_parents,
            num_uncompressed,
        } => {
            match get_args(*num_parents, *num_uncompressed, &parents[..]) {
                Some((num_parents, commits)) => {
                    let mut last_pass: Vec<Commit> =
                        commits.iter().map(|x| x.deref().clone()).collect();
                    let mut num_levels_processed = 0;
                    loop {
                        num_levels_processed += 1;
                        if num_levels_processed >= MAX_TREE_HEIGHT {
                            panic!("Max level of recursion reached: {}", num_levels_processed);
                        }

                        // parents[0] is the newest
                        let last_pass_oids =
                            build_tree(&repo, &to_refs(&last_pass)[..], num_parents)?;
                        if last_pass_oids.len() > 1 {
                            last_pass = last_pass_oids
                                .iter()
                                .flat_map(|oid| repo.find_commit(*oid).ok())
                                .collect();
                        } else {
                            return Ok(last_pass_oids);
                        }
                    }
                }
                None => Ok(vec![]),
            }
        }
    }
}

fn get_args<'a, T>(
    num_parents: Option<u8>,
    num_uncompressed: Option<u16>,
    parents: &'a [&'a T],
) -> Option<(u8, &'a [&'a T])> {
    if let Some(num_uncompressed) = num_uncompressed {
        if (num_uncompressed as usize) < parents.len() {
            // parents[0] is the newest
            Some((
                num_parents.unwrap_or(8),
                (&parents[(num_uncompressed as usize)..]),
            ))
        } else {
            None
        }
    } else {
        // Setting num_uncompressed to None/null means we don't compress any branches.
        None
    }
}

/// I couldn't find this in std:: probably because the lifetime makes it awkward to use
fn to_refs<T>(vec: &[T]) -> Vec<&T> {
    vec.iter().collect()
}

pub fn get_dura_snapshot_branches(repo: &Repository) -> Result<Vec<Branch>, Error> {
    filter_branches(repo, |name| {
        let splits: Vec<_> = name.split('/').collect();
        name.starts_with("dura/") && splits.len() == 2 && splits.get(1) != Some(&"cold")
    })
}

pub fn get_dura_cold_flat_branches(repo: &Repository) -> Result<Vec<Branch>, Error> {
    filter_branches(repo, |name| name.starts_with("dura/cold/") && name.split('/').count() == 3)
}

pub fn get_dura_cold_branch(repo: &Repository) -> Result<Branch, Error> {
    repo.find_branch("dura/cold", BranchType::Local)
}

fn filter_branches(repo: &Repository, predicate: fn(&str) -> bool) -> Result<Vec<Branch>, Error> {
    let ret: Vec<_> = repo
        .branches(Some(BranchType::Local))?
        .flat_map(|res| res.into_iter())
        .map(|tuple| {
            let (branch, _) = tuple;
            branch
        })
        .filter(|branch| match branch.name() {
            Ok(Some(name)) => predicate(name),
            _ => false,
        })
        .collect();

    Ok(ret)
}

fn sort(branches: &mut Vec<Branch>) {
    branches.sort_by(|a, b| {
        let a_time = a
            .get()
            .peel_to_commit()
            .map(|c| c.time())
            .unwrap_or_else(|_| Time::new(0, 0));
        let b_time = b
            .get()
            .peel_to_commit()
            .map(|c| c.time())
            .unwrap_or_else(|_| Time::new(0, 0));

        b_time.cmp(&a_time)
    });
}

/// Build a single layer of a tree. We're still not sure what we want out of a branch compaction
/// routine, so this is flexible enough to serve 2 use cases — a smaller amount of flat
/// "octopuses" (merge commits with >2 parents) or a hierarchical "B-tree" (merge commits
/// recursively rolling up into a single branch of cold branches).
fn build_tree<'a>(
    repo: &'a Repository,
    parent_commits: &[&'a Commit],
    num_parents: u8,
) -> Result<Vec<Oid>, Error> {
    let mut ret: Vec<Oid> = Vec::new();

    // parents[0] is the newest
    for parents in parent_commits.chunks(num_parents.into()) {
        if parents.is_empty() {
            break;
        }

        let message = "dura compacted commit";

        let oid = repo.commit(
            None,
            &parents[0].author(),
            &parents[0].committer(),
            message,
            &parents[0].tree()?,
            parents,
        )?;

        ret.push(oid);
    }

    Ok(ret)
}
