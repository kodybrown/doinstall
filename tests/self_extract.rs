//! Native acceptance stays inside disposable directories; PATH is never changed.
mod support;
use serde_json::{json, Value};
use std::{
  fs,
  path::{Path, PathBuf},
  process::{Command, Output},
};

struct Fixture {
  temp: tempfile::TempDir,
  config: PathBuf,
  delivery: PathBuf,
}
fn executable(stem: &str) -> String {
  format!("{stem}{}", if cfg!(windows) { ".exe" } else { "" })
}
impl Fixture {
  fn new() -> Self {
    let temp = support::temporary();
    let payload = temp.path().join("payload");
    fs::create_dir(&payload).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_doinstall"), payload.join(executable("app"))).unwrap();
    fs::create_dir(payload.join("nested 日本語")).unwrap();
    fs::create_dir(payload.join("empty-directory")).unwrap();
    fs::write(payload.join("nested 日本語/data.txt"), "data\n").unwrap();
    let config = temp.path().join("source.yaml");
    let yaml = json!({"schema":1,"application":{"id":"probe","name":"Probe","version":"1.23.2610.1413","build":{"stamp":"26287-0612"}},
      "platform":if cfg!(windows){"windows"}else if cfg!(target_os="macos"){"macos"}else{"linux"},
      "architecture":if cfg!(target_arch="aarch64"){"arm64"}else{"x64"},"payload":"payload",
      "runtime":{"interactive":false,"commands":[{"name":"probe","executable":executable("app")}],
        "values":{"install-dir":temp.path().join("installed"),"add-to-path":false}}});
    fs::write(&config, serde_saphyr::to_string(&yaml).unwrap()).unwrap();
    let delivery = temp.path().join("delivery");
    Self {
      temp,
      config,
      delivery,
    }
  }
  fn configure(&self, change: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_saphyr::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
    change(&mut value);
    fs::write(&self.config, serde_saphyr::to_string(&value).unwrap()).unwrap();
  }
  fn build(&self, arguments: &[&str]) -> Output {
    self.build_with_installer(Path::new(env!("CARGO_BIN_EXE_doinstall")), arguments)
  }
  fn build_with_installer(&self, installer: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_doinstall-builder"))
      .arg("--installer")
      .arg(installer)
      .arg("--config")
      .arg(&self.config)
      .arg("--output")
      .arg(&self.delivery)
      .arg("--self-extracting")
      .arg("--launcher-template")
      .arg(env!("CARGO_BIN_EXE_doinstall-launcher"))
      .args(arguments)
      .env("PATH", "") // Packaging must not invoke a compiler or installed archiver.
      .output()
      .unwrap()
  }
  fn launcher(&self) -> PathBuf {
    self.delivery.join(executable("probe-setup-v1.23.2610.1413-se"))
  }
  fn run(&self, directory: &Path, arguments: &[&str]) -> Output {
    Command::new(self.launcher())
      .arg("--extract-to")
      .arg(directory)
      .args(arguments)
      .current_dir(self.temp.path())
      .output()
      .unwrap()
  }
}
fn good(output: Output) -> Output {
  assert!(output.status.success(), "{output:?}");
  output
}

#[test]
fn standalone_launcher_installs_and_retained_sources_rerun_and_uninstall() {
  let f = Fixture::new();
  f.configure(|v| v["runtime"]["inputs"] = json!({"channel":{"type":"string","required":true,"default":"default"}}));
  let report = f.temp.path().join("result.json");
  good(f.build(&["--result-file", report.to_str().unwrap()]));
  let result: Value = serde_json::from_slice(&fs::read(report).unwrap()).unwrap();
  assert_eq!(PathBuf::from(result["FilePath"].as_str().unwrap()), f.launcher());
  assert_eq!(fs::read_dir(&f.delivery).unwrap().count(), 1);
  fs::remove_file(&f.config).unwrap();
  fs::remove_dir_all(f.temp.path().join("payload")).unwrap();
  let extract = f.temp.path().join("extracted 日本語 with spaces");
  good(f.run(
    &extract,
    &[
      "--non-interactive",
      "--set",
      "channel=space 日本語 \"quoted\" \\path",
    ],
  ));
  let inner = extract.join(executable("probe-setup-v1.23.2610.1413"));
  let yaml: Value =
    serde_saphyr::from_str(&fs::read_to_string(extract.join("probe-setup-v1.23.2610.1413.yaml")).unwrap()).unwrap();
  assert!(yaml.get("packaging").is_none());
  assert!(extract.join("payload/empty-directory").is_dir());
  assert_eq!(
    fs::read_to_string(extract.join("payload/nested 日本語/data.txt")).unwrap(),
    "data\n"
  );
  good(
    Command::new(&inner)
      .args([
        "--non-interactive",
        "--set",
        "channel=space 日本語 \"quoted\" \\path",
      ])
      .output()
      .unwrap(),
  );
  // Installed management names stay independent of distributable branding.
  let installed = f.temp.path().join("installed");
  let receipt: Value =
    serde_saphyr::from_str(&fs::read_to_string(installed.join("installer/installation.yaml")).unwrap()).unwrap();
  assert_eq!(receipt["values"]["channel"], "space 日本語 \"quoted\" \\path");
  good(
    Command::new(installed.join("installer").join(executable("installer")))
      .args(["uninstall", "--non-interactive"])
      .output()
      .unwrap(),
  );
  assert!(!installed.exists());
  assert!(inner.is_file());
  good(Command::new(inner).arg("--validate").output().unwrap());
}

#[test]
fn custom_names_matching_yaml_and_relative_config_arguments_work() {
  let f = Fixture::new();
  f.configure(|v| {
    v["packaging"] = json!({"installer-name":executable("custom-v3.2.1"),"launcher-name":executable("custom-se")})
  });
  good(f.build(&[]));
  let extract = f.temp.path().join("extract");
  let launcher = f.delivery.join(executable("custom-se"));
  good(
    Command::new(&launcher)
      .arg("--extract-to")
      .arg(&extract)
      .arg("--validate")
      .output()
      .unwrap(),
  );
  assert!(extract.join("custom-v3.2.1.yaml").is_file());
  assert!(extract.join(executable("custom-v3.2.1")).is_file());
  // Relative --config continues to resolve against the caller, not extraction.
  let override_config = f.temp.path().join("override.yaml");
  fs::copy(extract.join("custom-v3.2.1.yaml"), &override_config).unwrap();
  let mut yaml: Value = serde_saphyr::from_str(&fs::read_to_string(&override_config).unwrap()).unwrap();
  yaml["payload"] = json!(extract.join("payload"));
  fs::write(&override_config, serde_saphyr::to_string(&yaml).unwrap()).unwrap();
  good(
    Command::new(launcher)
      .current_dir(f.temp.path())
      .args([
        "--extract-to",
        "relative extraction",
        "--",
        "--config",
        "override.yaml",
        "--validate",
      ])
      .output()
      .unwrap(),
  );
  assert!(!f.temp.path().join("installed").exists());
}

#[test]
fn default_temp_directories_are_fresh_named_and_retained_even_on_installer_failure() {
  let f = Fixture::new();
  good(f.build(&[]));
  let mut retained = Vec::new();
  for _ in 0..2 {
    let result = Command::new(f.launcher()).arg("--unknown-installer-option").output().unwrap();
    assert_eq!(result.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&result.stderr);
    assert!(stderr.contains("Unknown argument"), "{stderr}");
    let path = PathBuf::from(stderr.lines().find_map(|line| line.strip_prefix("Installer source: ")).unwrap());
    assert!(path
      .file_name()
      .unwrap()
      .to_string_lossy()
      .starts_with("probe-setup-v1.23.2610.1413-se-"));
    assert!(path.join(executable("probe-setup-v1.23.2610.1413")).is_file());
    retained.push(path);
  }
  assert_ne!(retained[0], retained[1]);
  // The tests, not the product, own and remove these disposable source copies.
  for path in retained {
    fs::remove_dir_all(path).unwrap();
  }
}

#[test]
fn nonempty_extraction_and_corrupt_zip_do_not_run_installer() {
  let f = Fixture::new();
  good(f.build(&[]));
  let extract = f.temp.path().join("extract");
  fs::create_dir(&extract).unwrap();
  fs::write(extract.join("keep"), "user-owned").unwrap();
  assert!(!f.run(&extract, &[]).status.success());
  assert_eq!(fs::read_dir(&extract).unwrap().count(), 1);
  assert_eq!(fs::read_to_string(extract.join("keep")).unwrap(), "user-owned");
  let mut bytes = fs::read(f.launcher()).unwrap();
  // Unsigned Windows/Linux output has its footer at EOF. macOS output has an
  // ad-hoc signature; changing a payload byte is still caught by its ZIP hash.
  let archive = bytes.windows(4).rposition(|w| w == b"PK\x03\x04").unwrap();
  bytes[archive + 30] ^= 1;
  fs::write(f.launcher(), bytes).unwrap();
  assert!(!f.run(&f.temp.path().join("corrupt"), &[]).status.success());
  assert!(!f.temp.path().join("corrupt").exists());
  assert!(!f.temp.path().join("installed").exists());
}

#[test]
fn invalid_packaging_settings_fail_before_publication() {
  for settings in [
    json!({"installer-name":"../escape.exe"}),
    json!({"launcher-name":"CON.exe"}),
    json!({"launcher-name":"payload"}),
    json!({"installer-name":"installation.yaml"}),
    json!({"unknown-name":"bad"}),
    json!({"installer-name":executable("same"),"launcher-name":executable("same")}),
  ] {
    let f = Fixture::new();
    f.configure(|v| v["packaging"] = settings);
    assert!(!f.build(&[]).status.success());
    assert!(!f.delivery.exists());
  }
}

#[test]
fn native_probe_inherits_stdin_environment_cwd_and_returns_its_exit_code() {
  use std::io::Write;
  let f = Fixture::new();
  good(f.build_with_installer(&std::env::current_exe().unwrap(), &[]));
  let mut child = Command::new(f.launcher())
    .arg("--extract-to")
    .arg(f.temp.path().join("probe-extract"))
    .args(["--exact", "installer_probe", "--nocapture"])
    .env("DOINSTALL_TEST_PROBE", "日本語 environment")
    .env("DOINSTALL_TEST_CWD", f.temp.path())
    .current_dir(f.temp.path())
    .stdin(std::process::Stdio::piped())
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped())
    .spawn()
    .unwrap();
  child.stdin.take().unwrap().write_all(b"direct input\n").unwrap();
  let output = child.wait_with_output().unwrap();
  assert_eq!(output.status.code(), Some(37), "{output:?}");
  assert!(String::from_utf8_lossy(&output.stdout).contains("direct output"));
  assert!(String::from_utf8_lossy(&output.stderr).contains("direct error"));
}

#[test]
fn installer_probe() {
  if std::env::var("DOINSTALL_TEST_PROBE").is_err() {
    return;
  }
  use std::io::Read;
  assert_eq!(std::env::var("DOINSTALL_TEST_PROBE").unwrap(), "日本語 environment");
  assert_eq!(
    std::env::current_dir().unwrap(),
    PathBuf::from(std::env::var_os("DOINSTALL_TEST_CWD").unwrap())
  );
  let mut input = String::new();
  std::io::stdin().read_to_string(&mut input).unwrap();
  assert_eq!(input, "direct input\n");
  println!("direct output");
  eprintln!("direct error");
  std::process::exit(37);
}

#[cfg(windows)]
fn console_processes() -> Vec<u32> {
  let mut processes = vec![0; 256];
  let mut count = unsafe { windows::Win32::System::Console::GetConsoleProcessList(&mut processes) };
  if count as usize > processes.len() {
    processes.resize(count as usize, 0);
    count = unsafe { windows::Win32::System::Console::GetConsoleProcessList(&mut processes) };
  }
  processes.truncate(count as usize);
  processes
}
#[cfg(windows)]
#[test]
fn native_console_handoff() {
  if console_processes().is_empty() {
    assert!(
      std::env::var_os("DOINSTALL_REQUIRE_CONSOLE").is_none(),
      "Acceptance needs a real console"
    );
    return;
  }
  let f = Fixture::new();
  good(f.build_with_installer(&std::env::current_exe().unwrap(), &[]));
  let record = f.temp.path().join("launcher-pid");
  let mut child = Command::new(f.launcher())
    .arg("--extract-to")
    .arg(f.temp.path().join("console-extract"))
    .args(["--exact", "console_installer_probe", "--nocapture"])
    .env("DOINSTALL_CONSOLE_PID_FILE", &record)
    .spawn()
    .unwrap();
  fs::write(&record, child.id().to_string()).unwrap();
  assert!(child.wait().unwrap().success());
  assert_eq!(fs::read_to_string(record.with_extension("result")).unwrap(), "shared console");
}
#[cfg(windows)]
#[test]
fn console_installer_probe() {
  let Some(record) = std::env::var_os("DOINSTALL_CONSOLE_PID_FILE") else {
    return;
  };
  let record = PathBuf::from(record);
  for _ in 0..500 {
    if record.exists() {
      break;
    }
    std::thread::sleep(std::time::Duration::from_millis(10));
  }
  let launcher: u32 = fs::read_to_string(&record).unwrap().parse().unwrap();
  assert!(
    console_processes().contains(&launcher),
    "Installer and launcher must share the same Windows console"
  );
  fs::write(record.with_extension("result"), "shared console").unwrap();
}

#[test]
fn named_output_reuses_identical_bundle_and_refuses_changed_payload() {
  let f = Fixture::new();
  let parent = f.temp.path().join("named");
  let build = || {
    Command::new(env!("CARGO_BIN_EXE_doinstall-builder"))
      .arg("--installer")
      .arg(env!("CARGO_BIN_EXE_doinstall"))
      .arg("--config")
      .arg(&f.config)
      .arg("--output-parent")
      .arg(&parent)
      .arg("--self-extracting")
      .arg("--launcher-template")
      .arg(env!("CARGO_BIN_EXE_doinstall-launcher"))
      .output()
      .unwrap()
  };
  good(build());
  let package = fs::read_dir(&parent)
    .unwrap()
    .find_map(|entry| {
      let path = entry.unwrap().path();
      path.is_dir().then_some(path)
    })
    .unwrap();
  let artifact = package.join(executable("probe-setup-v1.23.2610.1413-se"));
  let before = fs::read(&artifact).unwrap();
  let repeated = good(build());
  assert!(String::from_utf8_lossy(&repeated.stdout).contains("Reused installer"));
  assert_eq!(fs::read(&artifact).unwrap(), before);
  fs::write(f.temp.path().join("payload/nested 日本語/data.txt"), "changed").unwrap();
  assert!(!build().status.success());
  assert_eq!(fs::read(artifact).unwrap(), before);
}

#[test]
fn zip_payload_is_retained_as_zip_and_unrecognized_templates_are_rejected() {
  use std::io::Write;
  let f = Fixture::new();
  let archive = f.temp.path().join("input.zip");
  let mut zip = zip::ZipWriter::new(fs::File::create(&archive).unwrap());
  zip
    .start_file(
      executable("app"),
      zip::write::SimpleFileOptions::default().unix_permissions(0o755),
    )
    .unwrap();
  zip.write_all(&fs::read(env!("CARGO_BIN_EXE_doinstall")).unwrap()).unwrap();
  zip.finish().unwrap();
  f.configure(|v| v["payload"] = json!("input.zip"));
  good(f.build(&[]));
  let extract = f.temp.path().join("zip-extract");
  good(f.run(&extract, &["--validate"]));
  assert_eq!(fs::read(extract.join("payload.zip")).unwrap(), fs::read(archive).unwrap());
  for (index, template) in [
    env!("CARGO_BIN_EXE_doinstall"),
    env!("CARGO_BIN_EXE_doinstall-builder"),
  ]
  .into_iter()
  .enumerate()
  {
    let invalid = f.temp.path().join(format!("invalid-template-{index}"));
    let result = Command::new(env!("CARGO_BIN_EXE_doinstall-builder"))
      .arg("--installer")
      .arg(env!("CARGO_BIN_EXE_doinstall"))
      .arg("--config")
      .arg(&f.config)
      .arg("--output")
      .arg(&invalid)
      .arg("--self-extracting")
      .arg("--launcher-template")
      .arg(template)
      .output()
      .unwrap();
    assert!(!result.status.success());
    assert!(!invalid.exists());
  }
}
