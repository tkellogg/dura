use crate::log::Operation;
use git2::{Oid, Repository};
use serde_json::map::Map;
use serde_json::value::from_value;
use serde_json::{json, Number, Value};
use std::collections::HashMap;
use std::io::{self, BufRead, Write};
use std::rc::Rc;

type FlexResult<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Reads an input stream that contains dura logs and enriches them with more analytics-ready info
/// like number of insertions & deletions. The result is written back out to an output stream.
pub fn get_snapshot_metrics(
    input: &mut dyn io::Read,
    output: &mut dyn io::Write,
) -> FlexResult<()> {
    let mut reader = io::BufReader::new(input);
    let mut writer = io::BufWriter::new(output);
    let mut line: u64 = 0; // for printing better error messages
    let mut repo_cache: HashMap<String, Rc<Repository>> = HashMap::new();
    loop {
        line += 1;
        let mut input_line = String::new();
        if reader.read_line(&mut input_line)? == 0 {
            return Ok(());
        }
        match scrape_log(input_line) {
            Ok(Some(mut output)) => {
                scrape_git(&mut output, &mut repo_cache)?;
                writeln!(&mut writer, "{}", output)?;
            }
            Ok(None) => {}
            // Seems like a good way to report errors, idk...
            Err(e) => eprintln!("line {}: {}", line, e),
        }
    }
}

/// Scrape information out of the snapshot log.
fn scrape_log(line: String) -> serde_json::Result<Option<Value>> {
    let input_val: Value = serde_json::from_str(line.as_str())?;
    let mut output_val = Value::Object(Map::new());

    if let Some(t) = input_val.get("time") {
        output_val["time"] = t.clone();
    }

    if let Some(op_value) = input_val.get("fields").and_then(|f| f.get("operation")) {
        match from_value(op_value.clone())? {
            Operation::Snapshot {
                repo,
                op: Some(op),
                error: _,
                latency,
            } => {
                output_val["repo"] = Value::String(repo);
                if let Some(latency) = Number::from_f64(latency as f64) {
                    output_val["latency"] = Value::Number(latency);
                }
                output_val["dura_branch"] = Value::String(op.dura_branch);
                output_val["commit_hash"] = Value::String(op.commit_hash);
                output_val["base_hash"] = Value::String(op.base_hash);
            }
            _ => return Ok(None),
        }
    } else {
        return Ok(None);
    }

    Ok(Some(output_val))
}

/// Use the info captured from scrape_log to open a repo and capture information about the commit
///
/// The repo_cache is retained between calls. This cache seems to cut runtime by 50% in a
/// completely non-scientific measure. It still seems to take unexpectedly long, probably because
/// it still has to open lots of files (for each commit & tree object) behind the scenes, and this
/// is inherently not cache-able.
fn scrape_git(
    value: &mut Value,
    repo_cache: &mut HashMap<String, Rc<Repository>>,
) -> Result<(), git2::Error> {
    if let Some(repo_path_value) = value.get("repo") {
        let repo_path = match repo_path_value.as_str() {
            Some(x) => Ok(x),
            None => Err(git2::Error::from_str(format!("Couldn't find 'repo' in JSON").as_str()))
        }?;
        let repo = match repo_cache.get(repo_path) {
            Some(repo) => Rc::clone(repo),
            None => {
                let repo = Rc::new(Repository::open(repo_path)?);
                repo_cache.insert(repo_path.to_string(), Rc::clone(&repo));
                repo
            }
        };
        let commit_opt = value
            .get("commit_hash")
            .and_then(|c| c.as_str())
            .and_then(|c| Oid::from_str(c).ok())
            .and_then(|c| repo.find_commit(c).ok());
        let parent_commit = commit_opt.as_ref().and_then(|c| c.parents().last());
        if let (Some(commit), Some(parent)) = (commit_opt, parent_commit) {
            let diff =
                repo.diff_tree_to_tree(Some(&parent.tree()?), Some(&commit.tree()?), None)?;
            let stats = diff.stats()?;
            value["num_files_changed"] = json!(stats.files_changed());
            value["insertions"] = json!(stats.insertions());
            value["deletions"] = json!(stats.deletions());

            let files: Vec<_> = diff
                .deltas()
                .flat_map(|d| d.new_file().path())
                .map(|p| p.to_str())
                .collect();
            value["files_changed"] = json!(files);
        };
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::metrics::scrape_log;

    #[test]
    fn scrape_log_happy_path() {
        // broken up into multiple lines to satisfy style checker, but serde_json will handle it
        // fine
        let line = r#"{"target":"dura::poller","file":"src/poller.rs",
            "name":"event src/poller.rs:70","level":"Level(Info)",
            "fields":{
                "message":"info_operation","operation":{"Snapshot":{
                    "error":null,"latency":0.00988253,"op":{
                        "base_hash":"3e8e8c99b5434e726b13f56ba00d139bab57d5eb",
                        "commit_hash":"3423d21a2937d95119982395bc1281d3d8ebe3b6",
                        "dura_branch":"dura/3e8e8c99b5434e726b13f56ba00d139bab57d5eb"
                    },
                    "repo":"/Users/timkellogg/code/dura"}
                }
            },"time":"2022-01-14T01:49:51.638031+00:00"
        }"#;

        let output = scrape_log(line.to_string()).unwrap().unwrap();

        assert_eq!(
            output["time"].as_str(),
            Some("2022-01-14T01:49:51.638031+00:00")
        );
        assert_eq!(output["repo"].as_str(), Some("/Users/timkellogg/code/dura"));
        assert_eq!(
            output["dura_branch"].as_str(),
            Some("dura/3e8e8c99b5434e726b13f56ba00d139bab57d5eb")
        );
        assert_eq!(
            output["commit_hash"].as_str(),
            Some("3423d21a2937d95119982395bc1281d3d8ebe3b6")
        );
        assert_eq!(
            output["base_hash"].as_str(),
            Some("3e8e8c99b5434e726b13f56ba00d139bab57d5eb")
        );
        let latency = output["latency"].as_f64().unwrap();
        assert!(latency < (0.00988253 + f32::EPSILON).into());
        assert!(latency > (0.00988253 - f32::EPSILON).into());
    }

    #[test]
    fn scrape_log_no_snapshot() {
        // broken up into multiple lines to satisfy style checker, but serde_json will handle it
        // fine
        let line = r#"{"target":"dura","file":"src/main.rs","name":"event src/main.rs:96",
            "level":"Level(Info)","fields":{"pid":5416},
            "time":"2022-01-14T01:45:37.469819+00:00"}"#;

        let output = scrape_log(line.to_string()).unwrap();

        assert_eq!(output, None);
    }
}
