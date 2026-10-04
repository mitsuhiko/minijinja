use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use minijinja::{Environment, TemplateSource};

use similar_asserts::assert_eq;

fn create_env() -> Environment<'static> {
    let mut env = Environment::new();
    let template = String::from("Hello World!");
    env.add_template_owned("hello", template).unwrap();
    env
}

#[test]
fn test_basic() {
    let env = create_env();
    let t = env.get_template("hello").unwrap();
    assert_eq!(t.render(()).unwrap(), "Hello World!");
}

#[test]
fn test_dynamic() {
    let mut env = Environment::new();
    let template = String::from("Hello World 2!");
    env.add_template_owned("hello2", template).unwrap();
    env.set_loader(|name| match name {
        "hello" => Ok(Some("Hello World!".to_string())),
        _ => Ok(None),
    });
    let t = env.get_template("hello").unwrap();
    assert_eq!(t.render(()).unwrap(), "Hello World!");
    let t = env.get_template("hello2").unwrap();
    assert_eq!(t.render(()).unwrap(), "Hello World 2!");
    let err = env.get_template("missing").unwrap_err();
    assert_eq!(
        err.to_string(),
        "template not found: template \"missing\" does not exist"
    );
}

#[test]
fn test_source_replace_static() {
    let mut env = Environment::new();
    env.add_template_owned("a", "1").unwrap();
    env.add_template_owned("a", "2").unwrap();
    let rv = env.get_template("a").unwrap().render(()).unwrap();
    assert_eq!(rv, "2");
}

#[test]
fn test_source_replace_dynamic() {
    let mut env = Environment::new();
    env.add_template("a", "1").unwrap();
    env.add_template("a", "2").unwrap();
    env.set_loader(|_| Ok(None::<String>));
    let rv = env.get_template("a").unwrap().render(()).unwrap();
    assert_eq!(rv, "2");
}

type Sources = Arc<Mutex<HashMap<&'static str, (usize, &'static str)>>>;

/// Creates an environment with a loader that serves templates from a
/// shared map.  Each template carries a version which is used for the
/// up-to-date check.
fn create_reloading_env(sources: &Sources, loads: &Arc<AtomicUsize>) -> Environment<'static> {
    let mut env = Environment::new();
    assert!(env.auto_reload());
    env.set_loader({
        let sources = sources.clone();
        let loads = loads.clone();
        move |name| {
            loads.fetch_add(1, Ordering::Relaxed);
            let Some((version, source)) = sources.lock().unwrap().get(name).copied() else {
                return Ok(None);
            };
            let sources = sources.clone();
            let name = name.to_string();
            Ok(Some(TemplateSource::new(source).with_uptodate_check(
                move || {
                    sources
                        .lock()
                        .unwrap()
                        .get(name.as_str())
                        .is_some_and(|x| x.0 == version)
                },
            )))
        }
    });
    env
}

#[test]
fn test_auto_reload() {
    let sources: Sources = Default::default();
    let loads = Arc::new(AtomicUsize::new(0));
    sources.lock().unwrap().insert("a", (0, "1"));
    let env = create_reloading_env(&sources, &loads);

    let old = env.get_template("a").unwrap();
    assert_eq!(old.render(()).unwrap(), "1");
    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "1");
    assert_eq!(loads.load(Ordering::Relaxed), 1);

    sources.lock().unwrap().insert("a", (1, "2"));
    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "2");
    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "2");
    assert_eq!(loads.load(Ordering::Relaxed), 2);

    // the old template is still usable
    assert_eq!(old.render(()).unwrap(), "1");
    assert_eq!(env.templates().count(), 1);

    sources.lock().unwrap().remove("a");
    let err = env.get_template("a").unwrap_err();
    assert_eq!(err.kind(), minijinja::ErrorKind::TemplateNotFound);
    assert_eq!(old.render(()).unwrap(), "1");
    assert_eq!(env.templates().count(), 0);
}

#[test]
fn test_auto_reload_disabled() {
    let sources: Sources = Default::default();
    let loads = Arc::new(AtomicUsize::new(0));
    sources.lock().unwrap().insert("a", (0, "1"));
    let mut env = create_reloading_env(&sources, &loads);
    env.set_auto_reload(false);
    assert!(!env.auto_reload());

    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "1");
    sources.lock().unwrap().insert("a", (1, "2"));
    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "1");
    assert_eq!(loads.load(Ordering::Relaxed), 1);

    env.clear_templates();
    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "2");
}

#[test]
fn test_auto_reload_dependencies() {
    let sources: Sources = Default::default();
    let loads = Arc::new(AtomicUsize::new(0));
    sources
        .lock()
        .unwrap()
        .insert("layout", (0, "[{% block body %}{% endblock %}]"));
    sources.lock().unwrap().insert("include", (0, "inc1"));
    sources.lock().unwrap().insert(
        "index",
        (
            0,
            "{% extends 'layout' %}{% block body %}{% include 'include' %}{% endblock %}",
        ),
    );
    let env = create_reloading_env(&sources, &loads);
    assert_eq!(
        env.get_template("index").unwrap().render(()).unwrap(),
        "[inc1]"
    );

    sources
        .lock()
        .unwrap()
        .insert("layout", (1, "<{% block body %}{% endblock %}>"));
    assert_eq!(
        env.get_template("index").unwrap().render(()).unwrap(),
        "<inc1>"
    );

    sources.lock().unwrap().insert("include", (1, "inc2"));
    assert_eq!(
        env.get_template("index").unwrap().render(()).unwrap(),
        "<inc2>"
    );
    assert_eq!(loads.load(Ordering::Relaxed), 5);
}

#[test]
fn test_auto_reload_ignores_static_templates() {
    let mut env = Environment::new();
    env.set_auto_reload(true);
    env.add_template_owned("a", "1").unwrap();
    env.set_loader(|_| Ok(Some("loaded".to_string())));
    assert_eq!(env.get_template("a").unwrap().render(()).unwrap(), "1");
}

#[test]
fn test_clone_keeps_loaded_templates() {
    let sources: Sources = Default::default();
    let loads = Arc::new(AtomicUsize::new(0));
    sources.lock().unwrap().insert("a", (0, "1"));
    let env = create_reloading_env(&sources, &loads);
    env.get_template("a").unwrap();
    let cloned = env.clone();
    assert_eq!(cloned.get_template("a").unwrap().render(()).unwrap(), "1");
    assert_eq!(loads.load(Ordering::Relaxed), 1);
}

#[test]
fn test_path_loader_reload() {
    let dir = std::env::temp_dir().join(format!(
        "minijinja-path-loader-reload-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("hello.txt");
    std::fs::write(&path, "Hello 1").unwrap();

    let mut env = Environment::new();
    env.set_loader(minijinja::path_loader(&dir));
    assert_eq!(
        env.get_template("hello.txt").unwrap().render(()).unwrap(),
        "Hello 1"
    );

    // the size changes so the stamp changes even with coarse mtimes
    std::fs::write(&path, "Hello 22").unwrap();
    assert_eq!(
        env.get_template("hello.txt").unwrap().render(()).unwrap(),
        "Hello 22"
    );

    std::fs::remove_file(&path).unwrap();
    assert!(env.get_template("hello.txt").is_err());
    std::fs::remove_dir_all(&dir).ok();
}
