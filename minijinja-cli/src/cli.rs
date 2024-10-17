use std::path::PathBuf;

use clap::{arg, command, value_parser, ArgAction, Command};

pub(super) fn make_command() -> Command {
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
                .default_value("auto")
                .env("MINIJINJA_FORMAT"),
            arg!(-a --autoescape <MODE> "reconfigures autoescape behavior")
                .value_parser(["auto", "html", "json", "none"])
                .default_value("auto")
                .env("MINIJINJA_AUTOESCAPE"),
            arg!(-D --define <EXPR> "defines an input variable (key=value)")
                .action(ArgAction::Append)
                .env("MINIJINJA_DEFINE"),
            arg!(--strict "disallow undefined variables in templates")
                .env("MINIJINJA_STRICT"),
            arg!(--"no-include" "Disallow includes and extending")
                .env("MINIJINJA_NO_INCLUDE"),
            arg!(--"no-newline" "Do not output a trailing newline")
                .env("MINIJINJA_NO_NEWLINE"),
            arg!(--"trim-blocks" "Enable the trim_blocks flag")
                .env("MINIJINJA_TRIM_BLOCKS"),
            arg!(--"lstrip-blocks" "Enable the lstrip_blocks flag")
                .env("MINIJINJA_LSTRIP_BLOCKS"),
            #[cfg(feature = "contrib")]
            arg!(--"py-compat" "Enables improved Python compatibility.  Enabling \
                this adds methods such as dict.keys and some others.")
                .env("MINIJINJA_PY_COMPAT"),
            arg!(-s --syntax <PAIR>... "Changes a syntax feature (feature=value) \
                [possible features: block-start, block-end, variable-start, variable-end, \
                comment-start, comment-end, line-statement-prefix, \
                line-statement-comment]")
                .env("MINIJINJA_SYNTAX"),
            arg!(--"safe-path" <PATH>... "Only allow includes from this path. Can be used multiple times.")
                .conflicts_with("no-include")
                .value_parser(value_parser!(PathBuf))
                .env("MINIJINJA_SAFE_PATH"),
            arg!(--env "Pass environment variables as ENV to the template")
                .env("MINIJINJA_ENV"),
            arg!(-E --expr <EXPR> "Evaluates an expression instead")
                .env("MINIJINJA_EXPR"),
            arg!(--"expr-out" <MODE> "Sets the expression output mode")
                .value_parser(["print", "json", "json-pretty", "status"])
                .default_value("print")
                .requires("expr")
                .env("MINIJINJA_EXPR_OUT"),
            arg!(--fuel <AMOUNT> "configures the maximum fuel")
                .value_parser(value_parser!(u64))
                .env("MINIJINJA_FUEL"),
            arg!(--dump <KIND> "dump internals of a template")
                .value_parser(["instructions", "ast", "tokens"])
                .env("MINIJINJA_DUMP"),
            #[cfg(feature = "repl")]
            arg!(--repl "starts the repl with the given data")
                .conflicts_with_all(["expr", "template"])
                .env("MINIJINJA_REPL"),
            #[cfg(feature = "completions")]
            arg!(--"generate-completion" <SHELL> "generate a completion script for the given shell")
                .value_parser([
                    "bash",
                    "elvish",
                    "fig",
                    "fish",
                    "nushell",
                    "powershell",
                    "zsh",
                ]),
            arg!(-o --output <FILENAME> "path to the output file")
                .default_value("-")
                .value_parser(value_parser!(PathBuf))
                .env("MINIJINJA_OUTPUT"),
            arg!(--select <SELECTOR> "select a path of the input data")
                .env("MINIJINJA_SELECT"),
            arg!(template: [TEMPLATE] "path to the input template")
                .default_value("-")
                .env("MINIJINJA_TEMPLATE"),
            arg!(data: [DATA] "path to the data file")
                .value_parser(value_parser!(PathBuf))
                .env("MINIJINJA_DATA"),
        ])
        .about("minijinja-cli is a command line tool to render or evaluate jinja2 templates.")
        .after_help("For more information see https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli/README.md")
}
