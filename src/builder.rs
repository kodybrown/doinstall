//! Build-machine packaging lives in this executable, never in the shipped installer.
#![allow(dead_code)]
mod binary;
mod build_info;
mod config;
mod console;
mod engine;
mod files;
mod host;
mod included_process;
mod launchers;
mod model;
mod package;
mod user_path;
mod workflow;
use anyhow::{bail, ensure, Context, Result};
use std::path::PathBuf;
fn main() {
  if let Err(error) = run() {
    eprintln!("doinstall-builder: {error:#}");
    std::process::exit(1);
  }
}
fn run() -> Result<()> {
  let mut arguments = std::env::args().skip(1);
  let mut installer = None;
  let mut config = None;
  let mut output = None;
  let mut named = false;
  let mut result_file = None;
  while let Some(argument) = arguments.next() {
    match argument.as_str() {
      "--installer" => {
        ensure!(installer.is_none(), "Duplicate --installer");
        installer = Some(PathBuf::from(config::argument_value(&mut arguments, &argument)?));
      }
      "--config" => {
        ensure!(config.is_none(), "Duplicate --config");
        config = Some(PathBuf::from(config::argument_value(&mut arguments, &argument)?));
      }
      "--output" | "--output-parent" => {
        ensure!(output.is_none(), "Specify one output directory");
        named = argument == "--output-parent";
        output = Some(PathBuf::from(config::argument_value(&mut arguments, &argument)?));
      }
      "--result-file" => {
        ensure!(result_file.is_none(), "Duplicate --result-file");
        result_file = Some(PathBuf::from(config::argument_value(&mut arguments, &argument)?));
      }
      "--help" | "-h" => {
        println!(
                    "doinstall-builder --installer FILE --config FILE (--output NEW_DIRECTORY | --output-parent DIRECTORY)\n  --output-parent DIRECTORY  Derive directory from application build metadata\n  --result-file FILE         Write absolute FilePath as JSON outside the package\nPaths resolve from the current directory; payload resolves from the YAML.\nThe result file parent must exist. Distribute the whole package, without the builder."
                );
        return Ok(());
      }
      _ => bail!("Unknown builder argument: {argument}. Use --help."),
    }
  }
  // Resolve caller paths once, before checking overlap or creating a package.
  let cwd = std::env::current_dir()?;
  let installer = files::absolute(&installer.context("--installer is required")?, &cwd)?;
  let config = files::absolute(&config.context("--config is required")?, &cwd)?;
  let output = files::absolute(&output.context("--output or --output-parent is required")?, &cwd)?;
  let result_file = result_file.map(|path| files::absolute(&path, &cwd)).transpose()?;
  let artifact = package::package(&config, &output, named, &installer, result_file.as_deref())?;
  if let Some(result) = result_file {
    files::write_atomic(&result, &serde_json::to_vec(&serde_json::json!({"FilePath":artifact}))?).with_context(
      || {
        format!(
          "Package is complete at {}; failed to write result file {}",
          artifact.display(),
          result.display()
        )
      },
    )?;
  }
  Ok(())
}
