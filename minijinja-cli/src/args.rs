use std::path::PathBuf;

use argument::{Error, Parser};

use crate::command::Arg;

/// The parsed command line arguments.
#[derive(Debug)]
pub struct Args {
    #[cfg(feature = "toml")]
    pub config_file: Option<PathBuf>,
    pub format: Option<String>,
    pub defines: Vec<String>,
    pub template: Option<String>,
    pub output: PathBuf,
    pub select: Option<String>,
    #[cfg_attr(not(feature = "toml"), allow(dead_code))]
    pub print_config: bool,
    pub syntax_help: bool,
    pub autoescape: Option<String>,
    pub strict: bool,
    pub no_newline: bool,
    pub trim_blocks: bool,
    pub lstrip_blocks: bool,
    #[cfg(feature = "contrib")]
    pub py_compat: bool,
    pub syntax: Vec<String>,
    pub env: bool,
    pub no_include: bool,
    pub safe_paths: Vec<PathBuf>,
    pub fuel: Option<u64>,
    pub expr: Option<String>,
    pub expr_out: Option<String>,
    pub dump: Option<String>,
    #[cfg(feature = "repl")]
    pub repl: bool,
    #[cfg(feature = "completions")]
    pub generate_completion: Option<argument_completions::Shell>,
    /// The template file.  This is `None` if `--template` is used and no
    /// template file was given.
    pub template_file: Option<String>,
    pub data_files: Vec<PathBuf>,
}

pub fn parse(mut p: Parser<Arg>) -> Result<Args, Error> {
    let mut args = Args {
        #[cfg(feature = "toml")]
        config_file: None,
        format: None,
        defines: Vec::new(),
        template: None,
        output: PathBuf::from("-"),
        select: None,
        print_config: false,
        syntax_help: false,
        autoescape: None,
        strict: false,
        no_newline: false,
        trim_blocks: false,
        lstrip_blocks: false,
        #[cfg(feature = "contrib")]
        py_compat: false,
        syntax: Vec::new(),
        env: false,
        no_include: false,
        safe_paths: Vec::new(),
        fuel: None,
        expr: None,
        expr_out: None,
        dump: None,
        #[cfg(feature = "repl")]
        repl: false,
        #[cfg(feature = "completions")]
        generate_completion: None,
        template_file: None,
        data_files: Vec::new(),
    };

    while let Some(arg) = p.param()? {
        match arg {
            #[cfg(feature = "toml")]
            Arg::ConfigFile => args.config_file = Some(p.raw_value()?.into()),
            Arg::Format => args.format = Some(p.value()?),
            Arg::Define => args.defines.push(p.value()?),
            Arg::Template => args.template = Some(p.value()?),
            Arg::Output => args.output = p.raw_value()?.into(),
            Arg::Select => args.select = Some(p.value()?),
            Arg::PrintConfig => args.print_config = true,
            Arg::Help => return Err(p.help()),
            Arg::LongHelp => return Err(p.long_help()),
            Arg::SyntaxHelp => args.syntax_help = true,
            Arg::Autoescape => args.autoescape = Some(p.value()?),
            Arg::Strict => args.strict = true,
            Arg::NoNewline => args.no_newline = true,
            Arg::TrimBlocks => args.trim_blocks = true,
            Arg::LstripBlocks => args.lstrip_blocks = true,
            #[cfg(feature = "contrib")]
            Arg::PyCompat => args.py_compat = true,
            Arg::Syntax => args.syntax.push(p.value()?),
            Arg::Env => args.env = true,
            Arg::NoInclude => args.no_include = true,
            Arg::SafePath => args.safe_paths.push(p.raw_value()?.into()),
            Arg::Fuel => args.fuel = Some(p.value()?),
            Arg::Expr => args.expr = Some(p.value()?),
            Arg::ExprOut => args.expr_out = Some(p.value()?),
            Arg::Dump => args.dump = Some(p.value()?),
            #[cfg(feature = "repl")]
            Arg::Repl => args.repl = true,
            #[cfg(feature = "completions")]
            Arg::GenerateCompletion => args.generate_completion = Some(p.value()?),
            Arg::TemplateFile => args.template_file = Some(p.value()?),
            Arg::DataFile => args.data_files.push(p.raw_value()?.into()),
        }
    }

    if !args.safe_paths.is_empty() && args.no_include {
        return Err(p.error("the argument '--safe-path <PATH>' cannot be used with '--no-include'"));
    }
    if args.expr_out.is_some() && args.expr.is_none() {
        return Err(p.error("the argument '--expr-out <MODE>' requires '--expr <EXPR>'"));
    }
    #[cfg(feature = "repl")]
    if args.repl {
        let conflict = if args.expr.is_some() {
            Some("--expr <EXPR>")
        } else if args.template.is_some() {
            Some("--template <TEMPLATE_STRING>")
        } else if args.template_file.is_some() {
            Some("[TEMPLATE_FILE]")
        } else {
            None
        };
        if let Some(conflict) = conflict {
            return Err(p.error(format!(
                "the argument '--repl' cannot be used with '{}'",
                conflict
            )));
        }
    }

    // the template file defaults to stdin unless a template string is given
    if args.template_file.is_none() && args.template.is_none() {
        args.template_file = Some("-".into());
    }

    Ok(args)
}
