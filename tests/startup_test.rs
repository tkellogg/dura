mod util;

use dura::config::Config;
use std::fs;
use dura::database::RuntimeDatabase;

#[test]
fn start_serve() {
    let mut dura = util::dura::Dura::new();
    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_runtime_db());
    assert_eq!(None, dura.get_runtime_db());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_db = dura.get_runtime_db();
    assert_ne!(None, runtime_db);
    assert_eq!(dura.pid(true), runtime_db.unwrap().pid);
}

#[test]
fn start_serve_with_null_pid_in_config() {
    let mut dura = util::dura::Dura::new();
    let mut runtime_db = RuntimeDatabase::empty();
    runtime_db.pid = None;
    dura.save_runtime_db(&runtime_db);

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_runtime_db());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_db = dura.get_runtime_db();
    assert_ne!(None, runtime_db);
    assert_eq!(dura.pid(true), runtime_db.unwrap().pid);
}

#[test]
fn start_serve_with_other_pid_in_config() {
    let mut dura = util::dura::Dura::new();
    let mut runtime_db = RuntimeDatabase::empty();
    runtime_db.pid = Some(12345);
    dura.save_runtime_db(&runtime_db);

    println!("db:: {:?}", dura.get_runtime_db());

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_runtime_db());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_db = dura.get_runtime_db();
    assert_ne!(None, runtime_db);
    assert_eq!(dura.pid(true), runtime_db.unwrap().pid);
}

#[test]
fn start_serve_with_invalid_json() {
    let mut dura = util::dura::Dura::new();
    let runtime_db_path = dura.runtime_db_path();
    Config::create_dir(runtime_db_path.as_path());
    fs::write(
        runtime_db_path,
        "{\"pid\":34725",
    )
    .unwrap();

    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_runtime_db());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let runtime_db = dura.get_runtime_db();
    assert_ne!(None, runtime_db);
    assert_eq!(dura.pid(true), runtime_db.unwrap().pid);
}

