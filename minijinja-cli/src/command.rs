/// This module defines the command-line interface for the CLI.
/// It is separated into its own file because it is used both by the main
/// application and by build.rs to generate shell completions and man pages.
use argument::{Cli, Opt, Pos, ValueHint};

const ADVANCED: &str = "Advanced";
const BEHAVIOR: &str = "Template Behavior";
const SECURITY: &str = "Security";

/// Supported formats
pub static SUPPORTED_FORMATS: &[(&str, &str, &[&str])] = &[
    #[cfg(feature = "cbor")]
    ("cbor", "CBOR", &["cbor"]),
    #[cfg(feature = "ini")]
    (
        "ini",
        "INI / Config",
        &["ini", "conf", "config", "properties"],
    ),
    #[cfg(not(feature = "json5"))]
    ("json", "JSON", &["json"]),
    #[cfg(feature = "json5")]
    ("json", "JSON / JSON5", &["json", "json5"]),
    #[cfg(feature = "querystring")]
    ("querystring", "Query String / Form Encoded", &["qs"]),
    #[cfg(feature = "toml")]
    ("toml", "TOML", &["toml"]),
    #[cfg(feature = "yaml")]
    ("yaml", "YAML 1.2", &["yaml", "yml"]),
];

/// The values for `--format` with their descriptions (and detected file
/// extensions) for the help page and completions.
static FORMAT_VALUES: &[(&str, &str)] = &[
    ("auto", "detect from the file extension"),
    #[cfg(feature = "cbor")]
    ("cbor", "CBOR (*.cbor)"),
    #[cfg(feature = "ini")]
    (
        "ini",
        "INI / Config (*.ini, *.conf, *.config, *.properties)",
    ),
    #[cfg(not(feature = "json5"))]
    ("json", "JSON (*.json)"),
    #[cfg(feature = "json5")]
    ("json", "JSON / JSON5 (*.json, *.json5)"),
    #[cfg(feature = "querystring")]
    ("querystring", "Query String / Form Encoded (*.qs)"),
    #[cfg(feature = "toml")]
    ("toml", "TOML (*.toml)"),
    #[cfg(feature = "yaml")]
    ("yaml", "YAML 1.2 (*.yaml, *.yml)"),
];

/// Identifies the options and arguments of the command line interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Arg {
    #[cfg(feature = "toml")]
    ConfigFile,
    Format,
    Define,
    Template,
    Output,
    Select,
    PrintConfig,
    Help,
    LongHelp,
    SyntaxHelp,
    Autoescape,
    Strict,
    NoNewline,
    TrimBlocks,
    LstripBlocks,
    #[cfg(feature = "contrib")]
    PyCompat,
    Syntax,
    Env,
    NoInclude,
    SafePath,
    Fuel,
    Expr,
    ExprOut,
    Dump,
    #[cfg(feature = "repl")]
    Repl,
    #[cfg(feature = "completions")]
    GenerateCompletion,
    TemplateFile,
    DataFile,
}

pub static CLI: Cli<Arg> = Cli::new("minijinja-cli")
    .version(env!("CARGO_PKG_VERSION"))
    .max_width(120)
    .before_help("minijinja-cli is a command line tool to render or evaluate jinja2 templates.")
    .about("Pass a template and optionally a file with template variables to render it to stdout.")
    .long_about(include_str!("long_help.txt"))
    .after_help("For a short help use --help, for extended help --long-help, and for help on syntax --syntax-help.")
    .args(&[
        Pos::new(Arg::TemplateFile, "TEMPLATE_FILE")
            .optional()
            .default_value("-")
            .value_hint(ValueHint::FilePath)
            .help("Path to the input template")
            .long_help("\
                This is the path to the input template in MiniJinja/Jinja2 syntax.  \
                If not provided this defaults to '-' which means the template is \
                loaded from stdin.  When the format is set to 'auto' which is the \
                default, the extension of the filename is used to detect the format.\n\n\
                \
                This argument can be set to an empty string when --template is provided \
                to allow a data file to be supplied."),
        Pos::new(Arg::DataFile, "DATA_FILE")
            .optional()
            .multiple()
            .value_hint(ValueHint::FilePath)
            .help("Path to one or more data files used as template context")
            .long_help("\
                Path to the data file in the given format.\n\n\
                \
                The data file is used to supply the context (variables) to the template. \
                Various file formats are supported.  When data is read from stdin (by using '-' \
                as file name), --format must be specified as auto detection is based on \
                file extensions.  Multiple files can be provided which are merged.  Later \
                files override files passed before."),
    ])
    .opts(&[
        #[cfg(feature = "toml")]
        Opt::new(Arg::ConfigFile)
            .long("config-file")
            .value("PATH")
            .value_hint(ValueHint::FilePath)
            .help("Alternative path to the config file.")
            .long_help("\
                Sets an alternative path to the config file.  By default the config file \
                is loaded from $HOME/.minijinja.toml.\n\n\
                \
                To see the possible config values use --print-config which will print the \
                current state of the config.\n\n\
                [env var: MINIJINJA_CONFIG_FILE]"),
        Opt::new(Arg::Format)
            .short('f')
            .long("format")
            .value("FORMAT")
            .possible_values_with_help(FORMAT_VALUES)
            .help("The format of the input data")
            .long_help("\
                Sets the format of the input data.\n\n\
                \
                By default (auto) the format is detected from the file extension of the \
                data file.  Auto detection (auto) is unavailable when stdin is used as \
                input format.\n\n\
                \
                For most formats the mapping is pretty straight forward as you expect.  The \
                only format worth calling out is INI where the unnamed section is always \
                called 'default' instead (in contrast to TOML which leaves it toplevel).\n\n\
                \
                [env var: MINIJINJA_FORMAT]"),
        Opt::new(Arg::Define)
            .short('D')
            .long("define")
            .value("EXPR")
            .value_hint(ValueHint::Other)
            .help("Defines an input variable (key=value / key:=json_value)")
            .long_help("\
                This defines an input variable for the template.  This is used in addition \
                to the input data file.  It supports three forms: key defines a single bool, \
                key=value defines a string value, key:=json_value defines a JSON/YAML value.  \
                The latter is useful to define strings, integers or simple array literals.  \
                It can be supplied multiple times to set more than one value.\n\n\
                \
                Examples:\n\
                -D name=Peter       defines a basic string\n\
                -D user_id:=42      defines an integer\n\
                -D is_active:=true  defines a boolean\n\
                -D is_true          shortform to define true boolean"),
        Opt::new(Arg::Template)
            .short('t')
            .long("template")
            .value("TEMPLATE_STRING")
            .value_hint(ValueHint::Other)
            .help("Render a string template")
            .long_help("\
                Renders a template from a string instead of the file given.\n\n\
                \
                This can be used as an alternative to the template file that is normally passed. \
                Note that this is different to --expr which evaluates expressions instead.\n\n\
                \
                Example: minijinja-cli --template='Hello {{ name }}' -Dname=World"),
        Opt::new(Arg::Output)
            .short('o')
            .long("output")
            .value("FILENAME")
            .default_value("-")
            .value_hint(ValueHint::FilePath)
            .help("Path to the output file")
            .long_help("\
                Path to the output file instead of stdout.\n\n\
                \
                By default templates will be rendered to stdout, but this can be used to directly write \
                into a target file instead.  The --no-newline flag can be used to disable the printing \
                of the trailing newline.  Files will be written atomically.  This means that if template \
                evaluation fails the original file remains."),
        Opt::new(Arg::Select)
            .long("select")
            .value("SELECTOR")
            .value_hint(ValueHint::Other)
            .help("Select a subset of the input data")
            .long_help("\
                Select a subset of the input data with a path expression.\n\n\
                \
                By default the input file is fed directly as context.  You can however also select a \
                sub-section of this file.  For instance if you have a TOML file where all variables \
                are placed in the values section you normally need to reference the values like so:\n\n\
                \
                {{ values.key }}\n\n\
                \
                If you however invoke minijinja-cli with --select=values you can directly reference \
                the keys:\n\n\
                \
                {{ key }}\n\n\
                \
                You can use dotted paths to select into sub sections (eg: --select=values.0.box)."),
        Opt::new(Arg::PrintConfig)
            .long("print-config")
            .help("Print out the loaded config"),
        Opt::new(Arg::Help)
            .short('h')
            .long("help")
            .help("Print short help (short texts)"),
        Opt::new(Arg::LongHelp)
            .long("long-help")
            .help("Print long help (extended, long explanation texts)"),
        Opt::new(Arg::SyntaxHelp)
            .long("syntax-help")
            .help("Print syntax help (primer on Jinja2/MiniJinja syntax)"),
        Opt::section(BEHAVIOR),
        Opt::new(Arg::Autoescape)
            .short('a')
            .long("autoescape")
            .value("MODE")
            .possible_values(&["auto", "html", "json", "none"])
            .help("Reconfigures autoescape behavior")
            .long_help("\
                Reconfigures autoescape behavior.  The default is 'auto' which means that \
                the file extension sets the auto escaping mode.\n\n\
                \
                html means that variables are escaped to HTML5 and XML rules.  json means \
                that output is safe for both JSON and YAML rules (eg: strings are formatted \
                as JSON strings etc.).  none disables escaping entirely.\n\n\
                \
                [env var: MINIJINJA_AUTOESCAPE]"),
        Opt::new(Arg::Strict)
            .long("strict")
            .help("Disallow undefined variables in templates")
            .long_help("\
                Disallow undefined variables in templates instead of rendering empty strings.\n\n\
                \
                By default a template will allow a singular undefined access.  This means that \
                for instance an unknown attribute to an object will render an empty string.  To \
                disable that you can use the strict mode in which case all undefined attributes \
                will error instead.\n\n\
                \
                [env var: MINIJINJA_STRICT]"),
        Opt::new(Arg::NoNewline)
            .short('n')
            .long("no-newline")
            .help("Do not output a trailing newline")
            .long_help("\
                Do not output a trailing newline after template evaluation.\n\n\
                \
                By default minijinja-cli will render a trailing newline when rendering.  This \
                flag can be used to disable that.\n\n\
                \
                [env var: MINIJINJA_NEWLINE]"),
        Opt::new(Arg::TrimBlocks)
            .long("trim-blocks")
            .help("Enable the trim-blocks flag")
            .long_help("\
                Enable the trim-blocks flag.\n\n\
                \
                This flag controls the trim-blocks template syntax feature.  When enabled trailing \
                whitespace including one newline is removed after a block tag.\n\n\
                \
                [env var: MINIJINJA_TRIM_BLOCKS]"),
        Opt::new(Arg::LstripBlocks)
            .long("lstrip-blocks")
            .help("Enable the lstrip-blocks flag")
            .long_help("\
                Enable the lstrip-blocks flag.\n\n\
                \
                This flag controls the lstrip-blocks template syntax feature.  When enabled leading \
                whitespace is removed before a block tag.\n\n\
                \
                [env var: MINIJINJA_LSTRIP_BLOCKS]"),
        #[cfg(feature = "contrib")]
        Opt::new(Arg::PyCompat)
            .long("py-compat")
            .help("Enables improved Python compatibility")
            .long_help("\
                Enables improved Python compatibility for templates.\n\n\
                \
                Enabling this adds methods such as dict.keys and some common others.  This is useful \
                when rendering templates that should be shared with Jinja2.\n\n\
                \
                [env var: MINIJINJA_PY_COMPAT]"),
        Opt::new(Arg::Syntax)
            .short('s')
            .long("syntax")
            .value("PAIR")
            .value_hint(ValueHint::Other)
            .help("Changes a syntax feature (feature=value) \
                [possible features: block-start, block-end, variable-start, variable-end, \
                comment-start, comment-end, line-statement-prefix, \
                line-statement-comment]")
            .long_help("\
                Changes a syntax feature.\n\n\
                \
                This allows reconfiguring syntax delimiters.  The flag can be provided multiple \
                times.  Each time it's feature=value where feature is the name of the syntax \
                delimiter to change.  The following list is the full list of syntax features \
                that can be reconfigured and the default value:\n\n\
                \
                block-start={%\n\
                block-end=%}\n\
                variable-start={{\n\
                variable-end=}}\n\
                comment-start={#\n\
                comment-end=%}\n\
                line-statement-prefix=\n\
                line-statement-comment=\n\n\
                \
                Example: minijinja-cli -svariable-start='${' -svariable-end='}'\n\n\
                \
                For environment variable usage split multiple config strings with whitespace.\n\n\
                \
                [env var: MINIJINJA_SYNTAX]"),
        Opt::new(Arg::Env)
            .long("env")
            .help("Pass environment variables as ENV to the template")
            .long_help("\
                Pass environment variables to the template and make them available under the ENV \
                variable within the template.\n\n\
                \
                [env var: MINIJINJA_ENV]"),
        Opt::section(SECURITY),
        Opt::new(Arg::NoInclude)
            .long("no-include")
            .help("Disallow includes and extending")
            .long_help("\
                Disallow includes and extending for security reasons.\n\n\
                \
                When this is enabled all inclusions and template extension features are disabled \
                entirely.  An alternative to disabling includes is to use the --safe-path feature \
                which allows white listing individual folders instead.\n\n\
                \
                [env var: MINIJINJA_INCLUDE]"),
        Opt::new(Arg::SafePath)
            .long("safe-path")
            .value("PATH")
            .value_hint(ValueHint::DirPath)
            .help("Only allow includes from this path")
            .long_help("\
                Only allow includes from this path.\n\n\
                \
                This can be used to better control where includes and layout extensions can load \
                templates from.  This can be supplied multiple times.\n\n\
                \
                When the environment variable is used to control this, use ':' to split multiple \
                paths on Unix and ';' on Windows (analog to the PATH environment variable).\n\n\
                \
                [env var: MINIJINJA_SAFE_PATH]"),
        Opt::new(Arg::Fuel)
            .long("fuel")
            .value("AMOUNT")
            .value_hint(ValueHint::Other)
            .help("Configures the maximum fuel")
            .long_help("\
                Sets the maximum fuel a template can consume.\n\n\
                \
                When fuel is set, every instruction consumes a certain amount of fuel. Usually 1, \
                some will consume no fuel. By default the engine has the fuel feature disabled (0). \
                To turn on fuel set something like 50000 which will allow 50.000 instructions to \
                execute before running out of fuel.\n\n\
                \
                This is useful as a basic security feature in CI pipelines or similar.\n\n\
                \
                [env var: MINIJINJA_FUEL]"),
        Opt::section(ADVANCED),
        Opt::new(Arg::Expr)
            .short('E')
            .long("expr")
            .value("EXPR")
            .value_hint(ValueHint::Other)
            .help("Evaluates an template expression")
            .long_help("\
                Evaluates a template expression instead of rendering a template.\n\n\
                \
                The value to the parameter is a template expression that is evaluated with the \
                context of the template and the result is emitted according to --expr-out.  The \
                default output mode is to print the result of the expression to stdout.\n\n\
                \
                Example: minijinja-cli --expr='1 < 10'"),
        Opt::new(Arg::ExprOut)
            .long("expr-out")
            .value("MODE")
            .possible_values(&["print", "json", "json-pretty", "status"])
            .help("The expression output mode")
            .long_help("\
                Sets the expression output mode for --expr.\n\n\
                \
                This defaults to 'print' which means that the expression's result is written to \
                stdout.  'json' (and 'json-pretty') does mostly the same but writes the result as \
                JSON result instead with one as a one-liner, the second in pretty printing. 'status' \
                exits the program with the result as a status code.  If the result is not a number it \
                will first convert the result into a bool and then exits as 0 if it was true, 1 \
                otherwise.\n\n\
                \
                [env var: MINIJINJA_EXPR_OUT]"),
        Opt::new(Arg::Dump)
            .long("dump")
            .value("KIND")
            .possible_values(&["instructions", "ast", "tokens"])
            .help("Dump internals of a template")
            .long_help("\
                Dump internals of a template to stdout.\n\n\
                \
                This feature is primarily useful to debug what is going on in a MiniJinja template. \
                'instructions' will dump out the bytecode that the engine generated, 'ast' dumps out \
                the AST in a text only format and 'tokens' will print a line per token of the template \
                after lexing."),
        #[cfg(feature = "repl")]
        Opt::new(Arg::Repl)
            .long("repl")
            .help("Starts the repl with the given data")
            .long_help("\
                Starts the read-eval loop with the given input data.\n\n\
                \
                This allows basic experimentation of MiniJinja expressions with some input data."),
        #[cfg(feature = "completions")]
        Opt::section("Shell Support"),
        #[cfg(feature = "completions")]
        Opt::new(Arg::GenerateCompletion)
            .long("generate-completion")
            .value("SH")
            .possible_values(argument_completions::Shell::NAMES)
            .help("Generate a completion script for the given shell")
            .long_help("\
                Generate a completion script for the given shell and print it to stdout.\n\n\
                \
                This completion script can be added to your shell startup to provide completions \
                for the minijinja-cli command."),
    ]);
