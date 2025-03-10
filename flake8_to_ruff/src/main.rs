//! Utility to generate Ruff's `pyproject.toml` section from a Flake8 INI file.
#![allow(
    clippy::collapsible_else_if,
    clippy::collapsible_if,
    clippy::implicit_hasher,
    clippy::match_same_arms,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::module_name_repetitions,
    clippy::must_use_candidate,
    clippy::similar_names,
    clippy::too_many_lines
)]
#![forbid(unsafe_code)]

use std::path::PathBuf;

use anyhow::Result;
use clap::Parser;
use configparser::ini::Ini;
use ruff::flake8_to_ruff;

#[derive(Parser)]
#[command(
    about = "Convert existing Flake8 configuration to Ruff.",
    long_about = None
)]
struct Cli {
    /// Path to the Flake8 configuration file (e.g., `setup.cfg`, `tox.ini`, or
    /// `.flake8`).
    #[arg(required = true)]
    file: PathBuf,
    /// Optional path to a `pyproject.toml` file, used to ensure compatibility
    /// with Black.
    #[arg(long)]
    pyproject: Option<PathBuf>,
    /// List of plugins to enable.
    #[arg(long, value_delimiter = ',')]
    plugin: Option<Vec<flake8_to_ruff::Plugin>>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Read the INI file.
    let mut ini = Ini::new_cs();
    ini.set_multiline(true);
    let config = ini.load(cli.file).map_err(|msg| anyhow::anyhow!(msg))?;

    // Read the pyproject.toml file.
    let black = cli
        .pyproject
        .map(flake8_to_ruff::parse_black_options)
        .transpose()?
        .flatten();

    // Create Ruff's pyproject.toml section.
    let pyproject = flake8_to_ruff::convert(&config, black.as_ref(), cli.plugin)?;
    println!("{}", toml_edit::easy::to_string_pretty(&pyproject)?);

    Ok(())
}
