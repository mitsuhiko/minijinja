use core::fmt;
use std::sync::{Arc, Mutex};

use minijinja::value::{from_args, Kwargs, Object, ObjectRepr};
use minijinja::{args, Environment, Error, ErrorKind, State, Value};
use syntect::highlighting::ThemeSet;
use syntect::html::highlighted_html_for_string;
use syntect::parsing::SyntaxSet;

struct Highlighter {
    ss: SyntaxSet,
    ts: ThemeSet,
    theme: Arc<Mutex<String>>,
}

impl fmt::Debug for Highlighter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "highlight")
    }
}

impl Highlighter {
    pub fn new() -> Highlighter {
        let ss = SyntaxSet::load_defaults_newlines();
        let ts = ThemeSet::load_defaults();
        Highlighter {
            ss,
            ts,
            theme: Mutex::new("InspiredGitHub".to_string()).into(),
        }
    }
}

impl Object for Highlighter {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn call(self: &Arc<Self>, state: &State<'_, '_>, args: &[Value]) -> Result<Value, Error> {
        let (lang, kwargs): (&str, Kwargs) = from_args(args)?;
        let caller: Value = kwargs.get("caller")?;
        let content = caller.call(state, args!())?;
        let mut content_str = content.as_str().ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                "call block did not return a string",
            )
        })?;
        if let Some(rest) = content_str.strip_prefix('\n') {
            content_str = rest;
        }
        let syntax = self.ss.find_syntax_by_token(lang).ok_or_else(|| {
            Error::new(
                ErrorKind::InvalidOperation,
                format!("unknown language {}", lang),
            )
        })?;
        kwargs.assert_all_used()?;
        let theme = self.theme.lock().unwrap();
        let rv = highlighted_html_for_string(
            content_str,
            &self.ss,
            syntax,
            &self.ts.themes[&theme as &str],
        )
        .map_err(|err| {
            Error::new(ErrorKind::InvalidOperation, "failed to syntax highlight").with_source(err)
        })?;
        Ok(Value::from_safe_string(rv))
    }

    fn call_method(
        self: &Arc<Self>,
        _state: &State<'_, '_>,
        method: &str,
        args: &[Value],
    ) -> Result<Value, Error> {
        match method {
            "set_theme" => {
                let (name,): (String,) = from_args(args)?;
                if !self.ts.themes.contains_key(&name) {
                    return Err(Error::new(
                        ErrorKind::InvalidOperation,
                        format!("unknown theme {}", name),
                    ));
                }
                *self.theme.lock().unwrap() = name;
                Ok(Value::UNDEFINED)
            }
            _ => Err(Error::from(ErrorKind::UnknownMethod)),
        }
    }
}

fn main() {
    let mut env = Environment::new();
    env.add_global("highlight", Value::from_object(Highlighter::new()));
    env.add_template("example.html", include_str!("example.html"))
        .unwrap();
    let template = env.get_template("example.html").unwrap();
    println!("{}", template.render(()).unwrap());
}
