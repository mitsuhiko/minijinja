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
                .value_parser(value_parser!(PathBuf)),
            arg!(--select <SELECTOR> "select a path of the input data"),
            arg!(template: [TEMPLATE] "path to the input template").default_value("-"),
            arg!(data: [DATA] "path to the data file").value_parser(value_parser!(PathBuf)),
        ])
        .about("minijinja-cli is a command line tool to render or evaluate jinja2 templates.")
        .after_help("For more information see https://github.com/mitsuhiko/minijinja/tree/main/minijinja-cli/README.md")
}
