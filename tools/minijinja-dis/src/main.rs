use std::error::Error;
use std::fs;
use std::io::{self, Read};
use std::path::PathBuf;

use argh::FromArgs;
use minijinja::machinery::{parse, CompiledTemplate, Instructions};

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

/// Utility to disassemble a template.
#[derive(FromArgs)]
pub struct Cli {
    /// disassemble the template. (default if `-x` not given)
    #[argh(switch, short = 'd')]
    disassemble: bool,

    /// dump the AST
    #[argh(switch, short = 'x')]
    dump_ast: bool,

    /// use a file instead of stdin
    #[argh(positional)]
    path: Option<PathBuf>,
}

fn execute() -> Result<(), Box<dyn Error>> {
    let cli: Cli = argh::from_env();

    let (source, filename) = match cli.path {
        Some(path) => (fs::read_to_string(&path)?, path.display().to_string()),
        None => {
            let mut source = String::new();
            io::stdin().read_to_string(&mut source)?;
            (source, "<stdin>".into())
        }
    };

    if cli.disassemble || !cli.dump_ast {
        let tmpl = CompiledTemplate::new(&filename, &source, Default::default())?;
        for (block_name, instructions) in tmpl.blocks.iter() {
            print_instructions(instructions, block_name);
        }
        print_instructions(&tmpl.instructions, "<root>");
    }

    if cli.dump_ast {
        if cli.disassemble {
            println!();
        }
        println!("{:#?}", parse(&source, &filename)?);
    }

    Ok(())
}

fn main() {
    if let Err(err) = execute() {
        eprintln!("Error: {err:#}");
        let mut source_opt = err.source();
        while let Some(source) = source_opt {
            eprintln!();
            eprintln!("caused by: {source:#}");
            source_opt = source.source();
        }
    }
}
