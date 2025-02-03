use std::collections::HashMap;
use std::fs;
use std::sync::Arc;

use minijinja::value::{Object, Value};
use minijinja::{context, Environment, State};

static TEMPLATE: &str = "\
{{ translate('GREETING') }}, {{ username }}!
{{ translate('GOODBYE') }}!\
 ";

#[derive(Debug, Default)]
struct Translations {
    strings: HashMap<Arc<str>, Arc<str>>,
}

impl Translations {
    fn load(lang: &str) -> Translations {
        eprintln!("[info] loading language {}", lang);
        let mut rv = Translations::default();
        rv.strings.extend(
            fs::read_to_string(format!("src/{}.txt", lang))
                .unwrap()
                .lines()
                .filter_map(|l| l.split_once('=').map(|(k, v)| (k.into(), v.into()))),
        );
        rv
    }
}

impl Object for Translations {}

fn translate(state: &State, key: &str) -> Option<Value> {
    let lang = state.lookup("LANG");
    let lang = lang.as_ref().and_then(|x| x.as_str()).unwrap();
    let cache_key = format!("translation-cache:{}", lang);
    let translations = state.get_or_set_temp_object(&cache_key, || Translations::load(lang));
    translations.strings.get(key).cloned().map(Value::from)
}

fn main() {
    let mut env = Environment::new();
    env.add_function("translate", translate);
    let template = env.template_from_str(TEMPLATE).unwrap();
    let rv = template
        .render(context! {
            LANG => std::env::var("LANG").as_deref().unwrap_or("en").split("_").next().unwrap(),
            username => "Peter"
        })
        .unwrap();
    println!("{}", rv);
}
