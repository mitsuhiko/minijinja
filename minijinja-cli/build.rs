use std::fs::create_dir_all;

pub mod cli {
    include!("src/command.rs");
}

fn main() -> std::io::Result<()> {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=src/command.rs");
    println!("cargo:rerun-if-changed=src/long_help.txt");
    println!("cargo:rerun-if-env-changed=ASSET_OUT_DIR");

    let out_dir = match std::env::var_os("ASSET_OUT_DIR") {
        Some(dir) => std::path::PathBuf::from(dir),
        None => return Ok(()),
    };

    let man_out_dir = out_dir.as_path().join("man");
    create_dir_all(&man_out_dir)?;
    argument_mangen::Man::new(&cli::CLI).generate_to(&man_out_dir)?;

    #[cfg(feature = "completions")]
    {
        let completions_out_dir = out_dir.as_path().join("completions");
        create_dir_all(&completions_out_dir)?;
        for shell in argument_completions::Shell::ALL {
            shell.generate_to(&cli::CLI, "minijinja-cli", &completions_out_dir)?;
        }
    }

    Ok(())
}
