use std::borrow::Cow;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{fs, io};

use anyhow::{bail, Context, Error};
use clap::{arg, command, value_parser, ArgAction, ArgMatches, Command};
use minijinja::machinery::{get_compiled_template, parse, tokenize, Instructions};
use minijinja::{
    context, AutoEscape, Environment, Error as MError, ErrorKind, UndefinedBehavior, Value,
};

#[cfg(feature = "repl")]
mod repl;

const STDIN: &str = "-";

#[cfg(not(feature = "json5"))]
use serde_json as preferred_json;
#[cfg(feature = "json5")]
use serde_json5 as preferred_json;

fn load_data(format: &str, path: &Path) -> Result<(BTreeMap<String, Value>, bool), Error> {
    let (contents, stdin_used) = if path == Path::new(STDIN) {
        (
            io::read_to_string(io::stdin()).context("unable to read data from stdin")?,
            true,
        )
    } else {
        (
            fs::read_to_string(path)
                .with_context(|| format!("unable to read data file '{}'", path.display()))?,
            false,
        )
    };
    let format = if format == "auto" {
        if stdin_used {
            bail!("auto detection does not work with data from stdin");
        }
        match path.extension().and_then(|x| x.to_str()) {
            Some("json") => "json",
            #[cfg(feature = "json5")]
            Some("json5") => "json",
            #[cfg(feature = "querystring")]
            Some("qs") => "querystring",
            #[cfg(feature = "yaml")]
            Some("yaml" | "yml") => "yaml",
            #[cfg(feature = "toml")]
            Some("toml") => "toml",
            #[cfg(feature = "cbor")]
            Some("cbor") => "cbor",
            _ => bail!("cannot auto detect format from extension"),
        }
    } else {
        format
    };

    let data = match format {
        "json" => preferred_json::from_str(&contents)?,
        #[cfg(feature = "querystring")]
        "querystring" => serde_qs::from_str(&contents)?,
        #[cfg(feature = "yaml")]
        "yaml" => serde_yaml::from_str(&contents)?,
        #[cfg(feature = "toml")]
        "toml" => toml::from_str(&contents)?,
        #[cfg(feature = "cbor")]
        "cbor" => ciborium::from_reader(contents.as_bytes())?,
        _ => unreachable!(),
    };
    Ok((data, stdin_used))
}

fn interpret_raw_value(s: &str) -> Result<Value, Error> {
    #[cfg(not(feature = "yaml"))]
    mod imp {
        pub use serde_json::from_str;
        pub const FMT: &str = "JSON/YAML";
    }
    #[cfg(feature = "yaml")]
    mod imp {
        pub use serde_yaml::from_str;
        pub const FMT: &str = "JSON";
    }
    imp::from_str::<Value>(s)
        .with_context(|| format!("invalid raw value '{}' (not valid {})", s, imp::FMT))
}

fn create_env(
    matches: &ArgMatches,
    cwd: PathBuf,
    allowed_template: Option<String>,
    stdin_used_for_data: bool,
) -> Environment<'static> {
    let mut env = Environment::new();
    env.set_debug(true);

    if let Some(fuel) = matches.get_one::<u64>("fuel") {
        if *fuel > 0 {
            env.set_fuel(Some(*fuel));
        }
    }

    minijinja_contrib::add_to_environment(&mut env);

    if matches.get_flag("env") {
        env.add_global("ENV", Value::from_iter(std::env::vars()));
    }

    let autoescape = matches.get_one::<String>("autoescape").unwrap().clone();
    env.set_auto_escape_callback(move |name| match autoescape.as_str() {
        "none" => AutoEscape::None,
        "html" => AutoEscape::Html,
        "json" => AutoEscape::Json,
        "auto" => match name.strip_suffix(".j2").unwrap_or(name).rsplit('.').next() {
            Some("htm" | "html" | "xml" | "xhtml") => AutoEscape::Html,
            Some("json" | "json5" | "yml" | "yaml") => AutoEscape::Json,
            _ => AutoEscape::None,
        },
        _ => unreachable!(),
    });
    env.set_undefined_behavior(if matches.get_flag("strict") {
        UndefinedBehavior::Strict
    } else {
        UndefinedBehavior::Lenient
    });
    env.set_path_join_callback(move |name, parent| {
        let p = if parent == STDIN {
            cwd.join(name)
        } else {
            Path::new(parent)
                .parent()
                .unwrap_or(Path::new(""))
                .join(name)
        };
        dunce::canonicalize(&p)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string()
            .into()
    });
    let cached_stdin = Mutex::new(None);
    env.set_loader(move |name| -> Result<Option<String>, MError> {
        if let Some(ref allowed_template) = allowed_template {
            if name != allowed_template {
                return Ok(None);
            }
        }

        if name == STDIN {
            if stdin_used_for_data {
                return Err(MError::new(
                    ErrorKind::InvalidOperation,
                    "cannot load template from stdin when data is from stdin",
                ));
            }

            let mut stdin = cached_stdin.lock().unwrap();
            if stdin.is_none() {
                *stdin = Some(io::read_to_string(io::stdin()).map_err(|err| {
                    MError::new(
                        ErrorKind::InvalidOperation,
                        "failed to read template from stdin",
                    )
                    .with_source(err)
                })?);
            }
            return Ok(stdin.clone());
        }

        match fs::read_to_string(name) {
            Ok(contents) => Ok(Some(contents)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(
                MError::new(ErrorKind::TemplateNotFound, "cannot find template").with_source(err),
            ),
        }
    });
    env
}

fn make_command() -> Command {
    command!()
        .args([
            arg!(-f --format <FORMAT> "the format of the input data")
                .value_parser([
                    "auto",
                    "json",
                    #[cfg(feature = "querystring")]
                    "querystring",
                    #[cfg(feature = "yaml")]
                    "yaml",
                    #[cfg(feature = "toml")]
                    "toml",
                    #[cfg(feature = "cbor")]
                    "cbor",
                ])
                .default_value("auto"),
            arg!(-a --autoescape <MODE> "reconfigures autoescape behavior")
                .value_parser(["auto", "html", "json", "none"])
                .default_value("auto"),
            arg!(-D --define <EXPR> "defines an input variable (key=value)")
                .action(ArgAction::Append),
            arg!(--strict "disallow undefined variables in templates"),
            arg!(--"no-include" "Disallow includes and extending"),
            arg!(--"no-newline" "Do not output a newline"),
            arg!(--env "Pass environment variables as ENV to the template"),
            arg!(-E --expr <EXPR> "Evaluates an expression instead"),
            arg!(--"expr-out" <MODE> "Sets the expression output mode")
                .value_parser(["print", "json", "json-pretty", "status"])
                .default_value("print")
                .requires("expr"),
            arg!(--fuel <AMOUNT> "configures the maximum fuel").value_parser(value_parser!(u64)),
            arg!(--dump <KIND> "dump internals of a template").value_parser(["instructions", "ast", "tokens"]),
            #[cfg(feature = "repl")]
            arg!(--repl "starts the repl with the given data")
                .conflicts_with_all(["expr", "template"]),
            arg!(template: [TEMPLATE] "path to the input template").default_value("-"),
            arg!(data: [DATA] "path to the data file").value_parser(value_parser!(PathBuf)),
        ])
        .about("minijinja-cli is a command line tool to render or evaluate jinja2 templates.")
        .after_help("For more information see https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli/README.md")
}

fn execute() -> Result<i32, Error> {
    let matches = make_command().get_matches();

    let format = matches.get_one::<String>("format").unwrap();
    let (base, stdin_used) = if let Some(data) = matches.get_one::<PathBuf>("data") {
        load_data(format, data)?
    } else {
        (Default::default(), false)
    };

    let mut defines = BTreeMap::new();
    if let Some(items) = matches.get_many::<String>("define") {
        for item in items {
            if let Some((key, raw_value)) = item.split_once(":=") {
                defines.insert(key, interpret_raw_value(raw_value)?);
            } else if let Some((key, string_value)) = item.split_once('=') {
                defines.insert(key, Value::from(string_value));
            } else {
                defines.insert(item, Value::from(true));
            }
        }
    }

    let cwd = std::env::current_dir()?;
    let ctx = context!(..defines, ..base);
    let template = match matches.get_one::<String>("template").unwrap().as_str() {
        STDIN => Cow::Borrowed(STDIN),
        rel_name => Cow::Owned(cwd.join(rel_name).to_string_lossy().to_string()),
    };
    let allowed_template = if matches.get_flag("no-include") {
        Some(template.to_string())
    } else {
        None
    };

    let no_newline = matches.get_flag("no-newline");

    let env = create_env(&matches, cwd, allowed_template, stdin_used);

    if let Some(expr) = matches.get_one::<String>("expr") {
        let rv = env.compile_expression(expr)?.eval(ctx)?;
        match matches.get_one::<String>("expr-out").unwrap().as_str() {
            "print" => println!("{}", rv),
            "json" => println!("{}", serde_json::to_string(&rv)?),
            "json-pretty" => println!("{}", serde_json::to_string_pretty(&rv)?),
            "status" => {
                return Ok(if let Ok(n) = i32::try_from(rv.clone()) {
                    n
                } else if rv.is_true() {
                    0
                } else {
                    1
                });
            }
            _ => unreachable!(),
        }
    } else if let Some(dump) = matches.get_one::<String>("dump") {
        match dump.as_str() {
            "ast" => {
                let tmpl = env.get_template(&template)?;
                println!("{:#?}", parse(tmpl.source(), tmpl.name())?);
            }
            "tokens" => {
                let tmpl = env.get_template(&template)?;
                let tokens: Result<Vec<_>, _> =
                    tokenize(tmpl.source(), false, Default::default()).collect();
                for (token, _) in tokens? {
                    println!("{:?}", token);
                }
            }
            "instructions" => {
                let tmpl = env.get_template(&template)?;
                let ctmpl = get_compiled_template(&tmpl);
                for (block_name, instructions) in ctmpl.blocks.iter() {
                    print_instructions(instructions, block_name);
                }
                print_instructions(&ctmpl.instructions, "<root>");
            }
            _ => unreachable!(),
        }
    } else if cfg!(feature = "repl") && matches.get_flag("repl") {
        #[cfg(feature = "repl")]
        {
            repl::run(env, ctx)?;
        }
    } else {
        let result = env.get_template(&template)?.render(ctx)?;
        if no_newline {
            print!("{result}");
        } else {
            println!("{result}");
        }
    }

    Ok(0)
}

fn print_instructions(instructions: &Instructions, block_name: &str) {
    println!("Block: {block_name:?}");
    for idx in 0.. {
        if let Some(instruction) = instructions.get(idx) {
            println!("  {idx:4}: {instruction:?}");
        } else {
            break;
        }
    }
}

pub fn print_error(err: &Error) {
    if let Some(err) = err.downcast_ref::<MError>() {
        eprintln!("template error: {err:#}");
    } else {
        eprintln!("error: {err}");
    }
    let mut source_opt = err.source();
    while let Some(source) = source_opt {
        eprintln!();
        if let Some(source) = source.downcast_ref::<MError>() {
            eprintln!("caused by template error: {source:#}");
        } else {
            eprintln!("caused by: {source}");
        }
        source_opt = source.source();
    }
}

fn main() {
    match execute() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            print_error(&err);
            std::process::exit(1);
        }
    }
}
