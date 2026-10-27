//! Self-hosted delivery must work through installed shims, without user PATH edits.
#[path = "../xtask/src/installer.rs"]
mod installer;
mod support;
use installer::executable;
use serde_json::Value;
use std::{
  fs,
  path::Path,
  process::{Command, Output},
};

fn good(mut command: Command) -> Output {
  let output = command.output().unwrap();
  assert!(output.status.success(), "{output:?}");
  output
}
fn command(path: impl AsRef<Path>, arguments: &[&str]) -> Command {
  let mut command = Command::new(path.as_ref());
  command.args(arguments);
  command
}
fn snapshot(directory: &Path) {
  fs::create_dir(directory).unwrap();
  for (name, source) in [
    ("doinstall", env!("CARGO_BIN_EXE_doinstall")),
    ("doinstall-builder", env!("CARGO_BIN_EXE_doinstall-builder")),
    ("doinstall-launcher", env!("CARGO_BIN_EXE_doinstall-launcher")),
    ("doinstall-server", env!("CARGO_BIN_EXE_doinstall-server")),
  ] {
    fs::copy(source, directory.join(executable(name))).unwrap();
  }
}

#[test]
fn self_installer_installs_tools_repackages_upgrades_and_uninstalls() {
  let temp = support::temporary();
  let tools = temp.path().join("build tools");
  snapshot(&tools);
  let delivery = temp.path().join("delivery");
  let launcher = installer::assemble(&tools, &delivery, "1.0.0", None).unwrap();
  let root = temp.path().join("installed 日本語");
  let bin = temp.path().join("commands");
  let source = temp.path().join("retained source");
  // Manually copied tools are unrecognized state, not an installation to adopt.
  fs::create_dir(&root).unwrap();
  fs::write(root.join("user-file"), "keep").unwrap();
  let mut install = command(&launcher, &["--non-interactive", "--set", "add-to-path=false"]);
  install
    .arg("--extract-to")
    .arg(temp.path().join("refused source"))
    .arg("--install-dir")
    .arg(&root)
    .arg("--bin-dir")
    .arg(&bin);
  assert!(!install.output().unwrap().status.success());
  assert_eq!(fs::read_to_string(root.join("user-file")).unwrap(), "keep");
  fs::remove_file(root.join("user-file")).unwrap();
  fs::remove_dir(&root).unwrap();
  let mut install = command(&launcher, &["--non-interactive", "--set", "add-to-path=false"]);
  install
    .arg("--extract-to")
    .arg(&source)
    .arg("--install-dir")
    .arg(&root)
    .arg("--bin-dir")
    .arg(&bin);
  good(install);
  assert!(source.join("payload").join(executable("doinstall-launcher")).is_file());
  assert!(!bin.join(executable("doinstall-launcher")).exists());
  for name in ["doinstall", "doinstall-builder", "doinstall-server"] {
    good(command(bin.join(executable(name)), &["--help"]));
  }
  let info: Value =
    serde_json::from_slice(&good(command(bin.join(executable("doinstall-builder")), &["--print-tool-paths"])).stdout)
      .unwrap();
  let actual = Path::new(info["Installer"].as_str().unwrap());
  assert!(actual.is_absolute());
  assert_eq!(
    fs::canonicalize(actual).unwrap().parent().unwrap().parent().unwrap(),
    fs::canonicalize(root.join("app")).unwrap()
  );
  // Package another self-installer using only installed real tools. No Cargo,
  // source checkout, globally installed tools, or project task settings are read.
  let second = installer::assemble(actual.parent().unwrap(), &delivery, "2.0.0", None).unwrap();
  let next_source = temp.path().join("next source");
  let mut upgrade = command(&second, &["--non-interactive", "--set", "add-to-path=false"]);
  upgrade
    .arg("--extract-to")
    .arg(&next_source)
    .arg("--install-dir")
    .arg(&root)
    .arg("--bin-dir")
    .arg(&bin);
  good(upgrade);
  let updated: Value =
    serde_json::from_slice(&good(command(bin.join(executable("doinstall-builder")), &["--print-tool-paths"])).stdout)
      .unwrap();
  assert_ne!(info["Installer"], updated["Installer"]);
  assert!(actual.is_file()); // Previous immutable builds remain until explicitly pruned.
  good(command(
    root.join("installer").join(executable("installer")),
    &["uninstall", "--non-interactive"],
  ));
  assert!(!root.exists());
  assert!(!bin.join(executable("doinstall-builder")).exists());
  assert!(source.join("payload").is_dir());
  assert!(next_source.join("payload").is_dir());
}

#[test]
fn tool_path_query_requires_real_adjacent_tools_and_accepts_no_packaging_flags() {
  let temp = support::temporary();
  let builder = temp.path().join(executable("doinstall-builder"));
  fs::copy(env!("CARGO_BIN_EXE_doinstall-builder"), &builder).unwrap();
  assert!(!command(&builder, &["--print-tool-paths"]).output().unwrap().status.success());
  let output = command(
    env!("CARGO_BIN_EXE_doinstall-builder"),
    &["--print-tool-paths", "--self-extracting"],
  )
  .output()
  .unwrap();
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("must be used alone"));
}

#[cfg(unix)]
#[test]
fn linked_builder_uses_real_siblings_for_query_and_default_launcher() {
  let temp = support::temporary();
  let tools = temp.path().join("real tools 日本語");
  snapshot(&tools);
  let commands = temp.path().join("commands");
  fs::create_dir(&commands).unwrap();
  let link = commands.join("builder-link");
  std::os::unix::fs::symlink(tools.join("doinstall-builder"), &link).unwrap();
  // The commands directory deliberately contains no runtime or launcher. Both
  // lookups must follow the builder link instead of searching PATH or cwd.
  let mut query = command(&link, &["--print-tool-paths"]);
  query.current_dir(&commands).env("PATH", "");
  let info: Value = serde_json::from_slice(&good(query).stdout).unwrap();
  assert_eq!(Path::new(info["Installer"].as_str().unwrap()), tools.join("doinstall"));
  assert_eq!(
    Path::new(info["LauncherTemplate"].as_str().unwrap()),
    tools.join("doinstall-launcher")
  );
  let config = temp.path().join("source.yaml");
  let yaml = serde_json::json!({"schema":1,
    "application":{"id":"linked-builder","name":"Linked builder","version":"1.0.0"},
    "platform":std::env::consts::OS,"architecture":installer::architecture().unwrap(),
    "payload":tools,"runtime":{"interactive":false,
      "commands":[{"name":"linked-builder","executable":"doinstall"}]}});
  fs::write(&config, serde_saphyr::to_string(&yaml).unwrap()).unwrap();
  let report = temp.path().join("result.json");
  let mut build = command(&link, &["--self-extracting"]);
  build
    .arg("--installer")
    .arg(tools.join("doinstall"))
    .arg("--config")
    .arg(&config)
    .arg("--output")
    .arg(temp.path().join("delivery"))
    .arg("--result-file")
    .arg(&report)
    .current_dir(&commands)
    .env("PATH", "");
  good(build);
  let result: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
  assert!(Path::new(result["FilePath"].as_str().unwrap()).is_file());
}
