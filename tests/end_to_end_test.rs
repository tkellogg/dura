mod util;

use std::fs;

use dura::config::Config;

#[test]
fn start_serve() {
    let mut dura = util::Dura::new();
    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_config());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let cfg = dura.get_config();
    assert_ne!(None, cfg);
    assert_eq!(dura.pid(true), cfg.unwrap().pid);
}

#[test]
fn start_serve_with_null_pid_in_config() {
    let mut cfg = Config::empty();
    cfg.pid = None;
    cfg.save();

    let mut dura = util::Dura::new();
    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_config());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let cfg = dura.get_config();
    assert_ne!(None, cfg);
    assert_eq!(dura.pid(true), cfg.unwrap().pid);
}

#[test]
fn start_serve_with_other_pid_in_config() {
    let mut dura = util::Dura::new();
    let mut cfg = Config::empty();
    cfg.pid = Some(12345);
    dura.save_config(&cfg);

    assert_eq!(None, dura.pid(true));
    assert_ne!(None, dura.get_config());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let cfg = dura.get_config();
    assert_ne!(None, cfg);
    assert_eq!(dura.pid(true), cfg.unwrap().pid);
}

#[test]
fn start_serve_with_invalid_json() {
    let mut dura = util::Dura::new();
    let cfg_path = dura.config_path();
    fs::write(cfg_path, "{\"pid\":34725,\"repos\":{}}Users/timkellogg/code/dura\":{}}}");

    assert_eq!(None, dura.pid(true));
    assert_eq!(None, dura.get_config());

    dura.start_async(&["serve"], true);
    dura.wait();

    assert_ne!(None, dura.pid(true));
    let cfg = dura.get_config();
    assert_ne!(None, cfg);
    assert_eq!(dura.pid(true), cfg.unwrap().pid);
}

