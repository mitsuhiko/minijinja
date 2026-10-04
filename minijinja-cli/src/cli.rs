use std::borrow::Cow;
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::{fs, io};

use anyhow::{bail, Context, Error};
use minijinja::machinery::{get_compiled_template, parse, tokenize, Instructions};
use minijinja::value::{merge_maps, ValueKind};
use minijinja::{context, Environment, Error as MError, ErrorKind, Value};

#[cfg(windows)]
use dunce::canonicalize;
#[cfg(not(windows))]
use std::fs::canonicalize;

use crate::args::Args;
use crate::command::{CLI, SUPPORTED_FORMATS};
use crate::config::Config;
use crate::convert::{from_minijinja, to_minijinja};
use crate::output::{Output, STDIN_STDOUT};

fn load_config(args: &Args) -> Result<Config, Error> {
    #[allow(unused_mut)]
    let mut config = None::<Config>;
    #[cfg(feature = "toml")]
    {
        let config_path = if let Some(path) = args.config_file.as_ref() {
            Some(Cow::Borrowed(path.as_path()))
        } else if let Some(var) = std::env::var_os("MINIJINJA_CONFIG_FILE") {
            Some(Cow::Owned(PathBuf::from(var)))
        } else {
            home_dir().map(|home_dir| Cow::Owned(home_dir.join(".minijinja.toml")))
        };

        if let Some(config_path) = config_path {
            if config_path.is_file() {
                config = Some(
                    Config::load_from_toml(&config_path)
                        .with_context(|| format!("unable to load '{}'", config_path.display()))?,
                );
            }
        }
    }
    let mut config = config.unwrap_or_default();
    config.update_from_env()?;
    config.update_from_args(args)?;
    Ok(config)
}

#[cfg(feature = "toml")]
fn home_dir() -> Option<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    std::env::var_os(var)
        .filter(|x| !x.is_empty())
        .map(PathBuf::from)
}

fn detect_format_from_path(path: &Path) -> Result<&'static str, Error> {
    if let Some(ext) = path.extension().and_then(|x| x.to_str()) {
        for (fmt, _, exts) in SUPPORTED_FORMATS {
            if exts.contains(&ext.to_ascii_lowercase().as_str()) {
                return Ok(fmt);
            }
        }
    }
    bail!("cannot auto detect format from extension");
}

fn load_data(
    format: &str,
    path: &Path,
    selector: Option<&str>,
) -> Result<(BTreeMap<String, Value>, bool), Error> {
    let (contents, stdin_used) = if path == Path::new(STDIN_STDOUT) {
        let mut buf = Vec::<u8>::new();
        io::stdin()
            .read_to_end(&mut buf)
            .context("unable to read data from stdin")?;
        (buf, true)
    } else {
        (
            fs::read(path)
                .with_context(|| format!("unable to read data file '{}'", path.display()))?,
            false,
        )
    };
    let format = if format == "auto" {
        if stdin_used {
            bail!("auto detection does not work with data from stdin");
        } else {
            detect_format_from_path(path)?
        }
    } else {
        format
    };

    let mut data: Value = match format {
        #[cfg(feature = "json5")]
        "json" => to_minijinja(&deser_json5::from_slice(&contents)?),
        #[cfg(not(feature = "json5"))]
        "json" => to_minijinja(&deser_json::from_slice(&contents)?),
        #[cfg(feature = "querystring")]
        "querystring" => Value::from(crate::querystring::from_bytes(&contents)?),
        #[cfg(feature = "yaml")]
        "yaml" => to_minijinja(&deser_yaml::from_slice(&contents)?),
        #[cfg(feature = "toml")]
        "toml" => to_minijinja(&deser_toml::from_slice(&contents)?),
        #[cfg(feature = "cbor")]
        "cbor" => to_minijinja(&deser_cbor::from_slice(&contents)?),
        #[cfg(feature = "ini")]
        "ini" => to_minijinja(&ini_sections(deser_ini::from_slice(&contents)?)),
        other => bail!("Unknown format '{}'", other),
    };

    if let Some(selector) = selector {
        for part in selector.split('.') {
            data = if let Ok(idx) = part.parse::<usize>() {
                data.get_item_by_index(idx)
            } else {
                data.get_attr(part)
            }
            .with_context(|| {
                format!(
                    "unable to select {:?} in {:?} (value was {})",
                    part,
                    selector,
                    data.kind()
                )
            })?
            .clone();
        }
    }

    Ok((
        into_context(data).context("failed to interpret input data as object")?,
        stdin_used,
    ))
}

/// Moves the keys before the first section of an INI file into the
/// `default` section.
#[cfg(feature = "ini")]
fn ini_sections(value: deser_value::Value) -> deser_value::Value {
    let Some(map) = value.as_map() else {
        return value;
    };
    let mut sections = deser_value::Map::new();
    let mut default = deser_value::Map::new();
    for (key, value) in map.iter() {
        if value.is_map() {
            sections.insert(key.clone(), value.clone());
        } else {
            default.insert(key.clone(), value.clone());
        }
    }
    if !default.is_empty() {
        let section = sections.get_or_insert_with("default", || deser_value::Map::new().into());
        if let Some(section) = section.as_map_mut() {
            for (key, value) in default.iter() {
                section.insert(key.clone(), value.clone());
            }
        }
    }
    sections.into()
}

/// Converts the data into the template context.
fn into_context(data: Value) -> Result<BTreeMap<String, Value>, Error> {
    if data.kind() != ValueKind::Map {
        bail!("expected a map, got {}", data.kind());
    }
    let mut rv = BTreeMap::new();
    for key in data.try_iter()? {
        let value = data.get_item(&key)?;
        let key = match key.as_str() {
            Some(key) => key.to_string(),
            None => key.to_string(),
        };
        rv.insert(key, value);
    }
    Ok(rv)
}

fn create_env(
    config: &Config,
    cwd: PathBuf,
    template_name: &str,
    template_source: Option<String>,
    stdin_used_for_data: bool,
) -> Result<Environment<'static>, Error> {
    let mut env = Environment::new();
    env.set_debug(true);
    config.apply_to_env(&mut env)?;

    env.set_path_join_callback(move |name, parent| {
        let p = if parent == STDIN_STDOUT {
            cwd.join(name)
        } else {
            Path::new(parent)
                .parent()
                .unwrap_or(Path::new(""))
                .join(name)
        };
        canonicalize(&p)
            .unwrap_or(p)
            .to_string_lossy()
            .to_string()
            .into()
    });

    let cached_stdin = Mutex::new(None);
    let safe_paths = config.safe_paths();
    let allow_include = config.allow_include();
    let template_name = template_name.to_string();
    env.set_loader(move |name| -> Result<Option<String>, MError> {
        if !allow_include && name != template_name {
            return Ok(None);
        }

        if name == STDIN_STDOUT {
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
        } else if name == template_name {
            if let Some(ref source) = template_source {
                return Ok(Some(source.clone()));
            }
        }

        let fs_name = Path::new(name);
        if !safe_paths.is_empty() && !safe_paths.iter().any(|x| fs_name.starts_with(x)) {
            return Err(MError::new(
                ErrorKind::InvalidOperation,
                "Cannot include template from non-trusted path",
            ));
        }

        match fs::read_to_string(name) {
            Ok(contents) => Ok(Some(contents)),
            Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(err) => Err(
                MError::new(ErrorKind::TemplateNotFound, "cannot find template").with_source(err),
            ),
        }
    });

    Ok(env)
}

#[cfg(feature = "completions")]
fn generate_completions(shell: argument_completions::Shell) -> Result<i32, Error> {
    print!("{}", shell.generate(&CLI, "minijinja-cli"));
    Ok(0)
}

fn dump_info(
    dump: &str,
    env: &Environment<'_>,
    template: &str,
    output: &mut Output,
) -> Result<(), Error> {
    match dump {
        "ast" => {
            let tmpl = env.get_template(template)?;
            writeln!(
                output,
                "{:#?}",
                parse(tmpl.source(), tmpl.name(), env.syntax().clone())?
            )?;
        }
        "tokens" => {
            let tmpl = env.get_template(template)?;
            let tokens: Result<Vec<_>, _> =
                tokenize(tmpl.source(), false, env.syntax().clone()).collect();
            for (token, _) in tokens? {
                writeln!(output, "{token:?}")?;
            }
        }
        "instructions" => {
            let tmpl = env.get_template(template)?;
            let ctmpl = get_compiled_template(&tmpl);
            for (block_name, instructions) in ctmpl.blocks.iter() {
                print_instructions(output, instructions, block_name)?;
            }
            print_instructions(output, &ctmpl.instructions, "<root>")?;
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn print_instructions(
    output: &mut Output,
    instructions: &Instructions,
    block_name: &str,
) -> Result<(), Error> {
    writeln!(output, "Block: {block_name:?}")?;
    for idx in 0.. {
        if let Some(instruction) = instructions.get(idx) {
            writeln!(output, "  {idx:4}: {instruction:?}")?;
        } else {
            break;
        }
    }
    Ok(())
}

fn print_expr_out(rv: Value, config: &Config, output: &mut Output) -> Result<i32, Error> {
    match config.expr_out() {
        "print" => writeln!(output, "{rv}")?,
        "json" => writeln!(output, "{}", deser_json::to_string(&from_minijinja(&rv)?)?)?,
        "json-pretty" => {
            let config = deser_json::SerializerConfig::builder()
                .pretty(deser_json::Indent::Spaces(2))
                .build();
            writeln!(output, "{}", config.to_string(&from_minijinja(&rv)?)?)?
        }
        "status" => {
            return Ok(if let Ok(n) = i32::try_from(rv.clone()) {
                n
            } else if rv.is_true() {
                0
            } else {
                1
            });
        }
        other => bail!("unknown expr-out '{}'", other),
    }
    Ok(0)
}

pub fn print_error(err: &Error) {
    eprintln!("error: {err}");
    if let Some(err) = err.downcast_ref::<MError>() {
        if err.name().is_some() {
            eprintln!("{}", err.display_debug_info());
        }
    }
    let mut source_opt = err.source();
    while let Some(source) = source_opt {
        eprintln!();
        eprintln!("caused by: {source}");
        if let Some(source) = source.downcast_ref::<MError>() {
            if source.name().is_some() {
                eprintln!("{}", source.display_debug_info());
            }
        }
        source_opt = source.source();
    }
}

#[cfg(feature = "toml")]
fn print_config(config: &Config) -> Result<i32, Error> {
    let out = deser_toml::to_string(config)?;
    println!("{out}");
    Ok(0)
}

fn repl_requested(_args: &Args) -> bool {
    #[cfg(feature = "repl")]
    {
        _args.repl
    }
    #[cfg(not(feature = "repl"))]
    {
        false
    }
}

pub fn execute() -> Result<i32, Error> {
    let args = CLI.run(crate::args::parse);
    let config = load_config(&args)?;

    if args.syntax_help {
        println!("{}", include_str!("syntax_help.txt"));
        return Ok(0);
    }

    #[cfg(feature = "completions")]
    {
        if let Some(shell) = args.generate_completion {
            return generate_completions(shell);
        }
    }
    #[cfg(feature = "toml")]
    {
        if args.print_config {
            return print_config(&config);
        }
    }

    let (base_ctx, stdin_used) = if !args.data_files.is_empty() {
        let mut contexts = Vec::with_capacity(args.data_files.len());
        let mut stdin_used = false;
        let select = args.select.as_deref();
        for data_file in &args.data_files {
            let (new_ctx, stdin_used_here) = load_data(config.format(), data_file, select)?;
            contexts.push(new_ctx);
            stdin_used = stdin_used || stdin_used_here;
        }
        (merge_maps(contexts), false)
    } else {
        (Default::default(), false)
    };

    let cwd = std::env::current_dir()?;
    let ctx = context!(..config.defines(), ..base_ctx);

    let (template_name, template_source) = match (
        args.template.as_ref(),
        args.template_file.as_deref(),
    ) {
        (None, Some(STDIN_STDOUT)) => (Cow::Borrowed(STDIN_STDOUT), None),
        (None, Some("")) => bail!("Empty template names are only valid with --template."),
        (None, Some(rel_name)) => (
            Cow::Owned(cwd.join(rel_name).to_string_lossy().to_string()),
            None,
        ),
        (Some(source), None | Some("")) => (Cow::Borrowed("<string>"), Some(source.clone())),
        _ => bail!("When --template is used, a template cannot be passed as argument (only an empty argument is allowed)."),
    };

    let mut output = Output::new(&args.output)?;

    let env = create_env(&config, cwd, &template_name, template_source, stdin_used)?;
    let mut exit_code = 0;

    if let Some(expr) = args.expr.as_ref() {
        let rv = env.compile_expression(expr)?.eval(ctx)?;
        exit_code = print_expr_out(rv, &config, &mut output)?;
    } else if let Some(dump) = args.dump.as_ref() {
        dump_info(dump, &env, &template_name, &mut output)?;
    } else if cfg!(feature = "repl") && repl_requested(&args) {
        #[cfg(feature = "repl")]
        {
            crate::repl::run(env, ctx)?;
        }
    } else {
        let result = env.get_template(&template_name)?.render(ctx)?;
        if !config.newline() {
            write!(&mut output, "{result}")?;
        } else {
            writeln!(&mut output, "{result}")?;
        }
    }

    output.commit()?;
    Ok(exit_code)
}
