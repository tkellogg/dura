use git2::{Branch, BranchType, Commit, Error, Oid, Repository, Tag, Time, Reference};
use std::ops::Deref;
use std::path::Path;
use std::cmp::min;
use std::collections::HashSet;

use crate::config::ConsolidateStrategy;
use crate::snapshots;

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
        } => {
            let mut to_remove = vec![];
            let mut has_excess = false;
            let res = match get_args(*num_parents, *num_uncompressed, &parents[..]) {
                Some((num_parents, commits)) => {
                    to_remove.extend(commits.iter().map(|c| c.id()));
                    let excess = match get_max_flat_node_index(&repo) {
                        Ok(max_index) => {
                            let branch_name = get_flat_branch_name(max_index);
                            let commit = repo.resolve_reference_from_short_name(
                                branch_name.as_str())?.peel_to_commit()?;
                            has_excess = true;
                            Some(commit)
                        }
                        Err(_) => None
                    };
                    build_tree(&repo, commits, num_parents, excess)?
                }
                None => vec![],
            };

            dbg!(res.len());
            tag_flat_nodes(&repo, &res[..], has_excess)?;
            delete_branches(&repo, &to_remove[..])?;

            Ok(res)
        }
        // Tree
        ConsolidateStrategy::Tree {
            num_parents,
            num_uncompressed,
        } => {
            let mut last_pass_oids: Vec<Oid> = vec![];
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
                        last_pass_oids = build_tree(&repo, &to_refs(&last_pass)[..], num_parents, None)?;
                        if last_pass_oids.len() > 1 {
                            last_pass = last_pass_oids
                                .iter()
                                .flat_map(|oid| repo.find_commit(*oid).ok())
                                .collect();
                        } else {
                            break;
                        }
                    }

                    tag_tree_node(&repo, &last_pass_oids[..])?;

                    Ok(last_pass_oids)
                }
                None => Ok(last_pass_oids),
            }
        }
    }
}

fn tag_flat_nodes(repo: &Repository, res: &[Oid], has_excess: bool) -> Result<(), Error> {
    let mut max_cold_index = get_max_flat_node_index(repo).unwrap_or(0);

    if max_cold_index > 0 && has_excess {
        // We won't overwrite the last commit if we don't rollback like this.
        max_cold_index -= 1;
    }

    let committer = snapshots::get_committer(repo)?;
    for commit in res.iter().rev() {
        max_cold_index += 1;
        repo.tag(
            get_flat_branch_name(max_cold_index).as_str(),
            repo.find_commit(*commit)?.as_object(),
            &committer,
            "dura cold storage",
            true,
        )?;
    }

    Ok(())
}

fn get_max_flat_node_index(repo: &Repository) -> Result<usize, Error> {
    repo
        .tag_names(Some("dura/cold/*"))?
        .iter()
        .flatten()
        .flat_map(|tag| tag.split("/").nth(2))
        .flat_map(|tag| tag.parse::<usize>().ok())
        .max()
        .ok_or(Error::from_str("No existing cold dura branches found ('dura/cold/*')"))
}

fn get_flat_branch_name(index: usize) -> String {
    format!("dura/cold/{}", index)
}

fn tag_tree_node(repo: &Repository, res: &[Oid]) -> Result<(), Error> {
    if let Some(oid) = res.get(0) {
        let committer = snapshots::get_committer(repo)?;

        repo.tag(
            "dura/cold",
            repo.find_commit(*oid)?.as_object(),
            &committer,
            "dura cold storage",
            true,
        )?;
    }
    Ok(())
}

fn get_branches_for_commits<'a>(repo: &'a Repository, commits: &'a [Oid]) -> Result<Vec<Reference<'a>>, Error> {
    let oids: HashSet<_> = commits.iter().collect();
    let ret = repo.references_glob("refs/heads/dura/*")?
        .flatten()
        .filter(|r| r.is_branch())
        .filter(|r| r.peel_to_commit().map(|c| oids.contains(&c.id())).unwrap_or(false))
        .collect();
    Ok(ret)
}

fn delete_branches(repo: &Repository, commits: &[Oid]) -> Result<(), Error> {
    for mut branch in get_branches_for_commits(repo, commits)? {
        branch.delete()?;
    }
    Ok(())
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

/// Get the branches generated by snapshots::capture()
pub fn get_dura_snapshot_branches(repo: &Repository) -> Result<Vec<Branch>, Error> {
    filter_branches(repo, |name| {
        let splits: Vec<_> = name.split('/').collect();
        name.starts_with("dura/") && splits.len() == 2 && splits.get(1) != Some(&"cold")
    })
}

/// Get the tags generated by ConsolidateStrategy::Flat
pub fn get_flat_tags(repo: &Repository) -> Result<Vec<Tag>, Error> {
    let vec = repo
        .tag_names(Some("dura/cold/*"))?
        .iter()
        .flat_map(|opt| match opt {
            Some(name) if name.split('/').count() == 3 => {
                // I'm using resolve_... because I don't know what tag_names returns. I guess I'm
                // lazy...
                repo.resolve_reference_from_short_name(name)
                    // This will swallow errors, but they should be mostly rare
                    .and_then(|r| r.peel_to_tag())
                    .ok()
            }
            _ => None,
        })
        .collect();

    Ok(vec)
}

/// Get the tag generated by ConsolidateStrategy::Tree
pub fn get_tree_tag(repo: &Repository) -> Result<Tag, Error> {
    repo.find_reference("refs/tags/dura/cold")?.peel_to_tag()
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

/// Groups commits together into a smaller number of merge commits.
///
/// **parent_commits** — the input list of commits to group. The caller is responsible for
/// finding these. Often they're snapshot branches, but they could also be intermediate commits
/// from building a tree.
///
/// **num_parents** — kinda non-intuitive wording, but it aligns to Git terminology. This is
/// the number of commits that are grouped together per merge commit. 
///
/// **excess_bucket** — a merge commit with potentially <=num_parents parents. The excess space
/// will be filled on this commit first before the other commits. A Oid representing this commit
/// will be returned (either literally this Oid, if nothing was added, or a new Oid).
///
/// Trivia: when num_parents>2, it's called an octopus commit.
///
/// Build a single layer of a tree. We're still not sure what we want out of a branch compaction
/// routine, so this is flexible enough to serve 2 use cases — a smaller amount of flat
/// "octopuses" (merge commits with >2 parents) or a hierarchical "B-tree" (merge commits
/// recursively rolling up into a single branch of cold branches).
fn build_tree<'a>(
    repo: &'a Repository,
    parent_commits: &[&'a Commit],
    num_parents: u8,
    excess_bucket: Option<Commit>,
) -> Result<Vec<Oid>, Error> {
    let mut ret: Vec<Oid> = Vec::new();
    let mut excess_oid = None;
    // parent_commits[0] is newest commit

    // fill excess first.
    if let Some(ref excess) = excess_bucket {
        dbg!(&excess, excess.parents().collect::<Vec<_>>());
        let num_excess = min(num_parents as i64 - excess.parents().len() as i64, parent_commits.len() as i64);
        if num_excess > 0 {
            let excess_parents = &parent_commits[parent_commits.len()-(num_excess as usize)..];
            let mut parent_set: Vec<_> = excess_parents.iter().map(|x| *x).collect();
            // this little dance seems to resolve lifetime issues
            let cloned: Vec<_> = excess.parents().map(|x| x.clone()).collect();
            parent_set.append(&mut cloned.iter().map(|x| x).collect());

            // re-do the commit
            let oid = make_compacted_commit(repo, &parent_set[..])?;
            println!("Added commits to existing branch; old_hash: {:?}, new_hash: {:?}",
                     &excess.id(), oid);

            excess_oid = Some(oid);
        }
    }

    // We want to do chunks over parent_commits, but .chunks() leaves the "extras" for the final
    // chunk. That doesn't work because the last chunk is the oldest chunk, so should be full. We
    // do this dance here to manually process teh first chunk, only if not full.
    let unfinished_size = parent_commits.len() % num_parents as usize;
    if unfinished_size > 0 {
        ret.push(make_compacted_commit(repo, &parent_commits[0..unfinished_size])?)
    }

    // parents[0] is the newest
    for parents in parent_commits[unfinished_size..].chunks(num_parents.into()) {
        if parents.is_empty() {
            break;
        }

        ret.push(make_compacted_commit(repo, parents)?);
    }

    // it'd oldest, so it has to be pushed last
    if let Some(oid) = excess_oid {
        ret.push(oid);
    }

    Ok(ret)
}

fn make_compacted_commit(repo: &Repository, parents: &[&Commit]) -> Result<Oid, Error> {
    let message = "dura compacted commit";

    let oid = repo.commit(
        None,
        &parents[0].author(),
        &parents[0].committer(),
        message,
        &parents[0].tree()?,
        parents,
    )?;
    Ok(oid)
}
