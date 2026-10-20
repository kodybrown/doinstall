mod build_info;
mod config;
mod console;
mod engine;
mod files;
mod host;
mod included_process;
mod launchers;
mod model;
mod settings_files;
mod tool_version;
mod tui;
mod user_path;
mod workflow;

use anyhow::{ensure, Context, Result};
use std::path::PathBuf;
fn main() {
  if tool_version::print(&std::env::args().skip(1).collect::<Vec<_>>(), "doinstall") {
    return;
  }
  let default_panic = std::panic::take_hook();
  std::panic::set_hook(Box::new(move |info| {
    if !console::report_worker_panic(info) {
      default_panic(info);
    }
  }));
  // TUI cancellation returns through backend cleanup. During an included tool,
  // plain interruption lets its owned process tree stop and its journal persist.
  // Other plain phases retain immediate interruption and activation retry records.
  if let Err(error) = ctrlc::set_handler(|| {
    if !console::request_tui_cancel() && !included_process::request_cancel() {
      console::canceled();
      std::process::exit(130);
    }
  }) {
    console::error(&error.into());
    std::process::exit(1);
  }
  if let Err(error) = run() {
    if error.is::<console::Canceled>() || error.is::<included_process::Cancelled>() {
      console::canceled();
      std::process::exit(130);
    }
    console::error(&error);
    std::process::exit(1);
  }
}
fn run() -> Result<()> {
  let arguments: Vec<String> = std::env::args().skip(1).collect();
  let options = config::Options::parse(arguments)?;
  if options.help {
    println!(
            "doinstall [install|uninstall] [--config FILE]\n  --interactive | --non-interactive  Override installer.yaml prompt mode\n  --plain                      Use line prompts instead of the interactive dialog\n  --install-dir ABSOLUTE_PATH   Installation root\n  --bin-dir ABSOLUTE_PATH       Enable an additional command in this directory\n  --additional-command         Enable the additional command (default: false)\n  --add-to-path                Add the selected command directory to user PATH (Windows)\n  --set NAME=VALUE              Override a YAML input\n  --profile NAME               Select environment defaults\n  --prune-old-versions          Keep new and previously active builds\n  --desktop-shortcuts --start-menu-shortcuts --local-shortcuts\n  --shortcut-name NAME         Name for shortcuts using ${{shortcut-name}}, without extension\n  --start-menu-nested          Nest Windows shortcuts under name/name.lnk\n  --leave-settings | --remove-settings  Uninstall settings policy\n  --validate                   Validate package without installing or prompting\nInteractive by default; use --non-interactive for automation. Rollback and version switching are not supported."
        );
    return Ok(());
  }
  let executable = std::env::current_exe()?;
  let config_file = match &options.config {
    Some(path) => files::absolute(path, &std::env::current_dir()?)?,
    None => executable.with_file_name(files::installer_yaml_name(
      executable
        .file_name()
        .and_then(|n| n.to_str())
        .context("Installer filename is not UTF-8")?,
    )),
  };
  if options.uninstall {
    ensure!(
      !options.validate,
      "--validate checks an installer package and cannot be used with uninstall"
    );
    let root = if let Some(value) = options.values.get("install-dir") {
      PathBuf::from(config::text(value)?)
    } else {
      ensure!(
        executable
          .parent()
          .and_then(|p| p.file_name())
          .is_some_and(|n| n == "installer"),
        "Package uninstall requires --install-dir"
      );
      executable
        .parent()
        .and_then(|p| p.parent())
        .context("Cannot locate installation root")?
        .to_path_buf()
    };
    let root = files::absolute(&root, &std::env::current_dir()?)?;
    if !options.plain && options.interactive != Some(false) {
      let package: model::Package = files::read_yaml(&root.join("installer/installer.yaml"))?;
      if options.prompts(&package)? {
        return tui::uninstall(root, package, options);
      }
    }
    engine::uninstall(&root, &options)
  } else {
    ensure!(!options.settings_explicit, "Settings flags are only valid with uninstall");
    let package = files::read_yaml(&config_file)?;
    if !options.plain && options.prompts(&package)? {
      return tui::install(config_file, package, options);
    }
    engine::install(&config_file, package, &options)
  }
}
