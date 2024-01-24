use std::fs::create_dir_all;

use clap::ValueEnum;
use clap_complete::Shell;
use clap_complete_fig::Fig;
use clap_complete_nushell::Nushell;

pub mod cli {
    include!("src/cli.rs");
}

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");

    let out_dir = match std::env::var_os("ASSET_OUT_DIR") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => return Ok(()),
    };

    let mut cli = cli::make_command();

    let man = clap_mangen::Man::new(cli.clone());
    let mut man_buffer: Vec<u8> = Default::default();
    man.render(&mut man_buffer)?;

    let man_out_dir = out_dir.as_path().join("man");
    create_dir_all(&man_out_dir)?;
    std::fs::write(man_out_dir.join("minijinja-cli.1"), man_buffer)?;

    let completions_out_dir = out_dir.as_path().join("completions");
    create_dir_all(&completions_out_dir)?;

    for shell in Shell::value_variants() {
        clap_complete::generate_to(*shell, &mut cli, "minijinja-cli", &completions_out_dir)?;
    }
    clap_complete::generate_to(Nushell, &mut cli, "minijinja-cli", &completions_out_dir)?;
    clap_complete::generate_to(Fig, &mut cli, "minijinja-cli", &completions_out_dir)?;

    Ok(())
}
