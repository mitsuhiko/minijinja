mod args;
mod cli;
mod command;
mod config;
mod convert;
mod output;
#[cfg(feature = "querystring")]
mod querystring;
#[cfg(feature = "repl")]
mod repl;

fn main() {
    match cli::execute() {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            cli::print_error(&err);
            std::process::exit(1);
        }
    }
}
