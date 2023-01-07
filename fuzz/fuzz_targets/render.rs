#![no_main]
use std::collections::BTreeMap;

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use serde::Serialize;

#[derive(Debug, Serialize, Arbitrary)]
enum Value {
    None,
    Bool(bool),
    Integer(i64),
    Float(f64),
    String(String),
    List(Vec<Value>),
    Map(BTreeMap<String, Value>),
}

fuzz_target!(|data: (&str, Vec<(&str, &str)>, Value)| {
    let (root, includes, value) = data;

    let mut env = minijinja::Environment::new();

    if env.add_template("fuzz", root).is_err() {
        return;
    }

    for (name, data) in includes {
        let _ = env.add_template(name, data);
    }

    let tmpl = env.get_template("fuzz").unwrap();
    tmpl.render(&value).ok();
});
