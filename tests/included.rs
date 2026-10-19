//! Included-tool acceptance uses a native test probe and disposable paths only.
mod support;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
  fs,
  path::{Path, PathBuf},
  process::Command,
};

struct Fixture {
  _temp: tempfile::TempDir,
  config: PathBuf,
  root: PathBuf,
  managed: PathBuf,
  sentinel: PathBuf,
  payload: PathBuf,
}
impl Fixture {
  fn new(phase: &str) -> Self {
    let temp = support::temporary();
    let root = temp.path().join("installed");
    let managed = temp.path().join("service.conf");
    let sentinel = temp.path().join("fail");
    let payload = temp.path().join("payload");
    fs::create_dir(&payload).unwrap();
    let executable = if cfg!(windows) { "probe.exe" } else { "probe" };
    fs::copy(std::env::current_exe().unwrap(), payload.join(executable)).unwrap();
    let hash = format!("{:x}", Sha256::digest(fs::read(payload.join(executable)).unwrap()));
    let config = temp.path().join("installer.yaml");
    let call = json!({"tool":"probe", "arguments":["--exact","tool_probe","--nocapture"], "environment":{"PROBE_CONFIG":managed,"PROBE_SENTINEL":sentinel}, "timeout-seconds":10});
    let package = json!({"schema":1,"application":{"id":"probe","name":"Probe","version":"1.0"},
      "platform":if cfg!(windows){"windows"}else if cfg!(target_os="macos"){"macos"}else{"linux"},
      "architecture":if cfg!(target_arch="aarch64"){"arm64"}else{"x64"},"payload":"payload",
      "runtime":{"interactive":false,"values":{"install-dir":root,"add-to-path":false},
        "tools":{"probe":{"executable":executable,"sha256":hash}},
        "managed-files":[managed], "lifecycle":{phase:[call]}}});
    fs::write(&config, serde_saphyr::to_string(&package).unwrap()).unwrap();
    Self {
      _temp: temp,
      config,
      root,
      managed,
      sentinel,
      payload,
    }
  }
  fn run(&self, args: &[&str]) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_doinstall"))
      .arg("--config")
      .arg(&self.config)
      .args(args)
      .env("PATH", "")
      .output()
      .unwrap()
  }
  fn configure(&self, update: impl FnOnce(&mut Value)) {
    let mut value: Value = serde_saphyr::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
    update(&mut value);
    fs::write(&self.config, serde_saphyr::to_string(&value).unwrap()).unwrap();
  }
  fn receipt(&self) -> Value {
    yaml(&self.root.join("installer/installation.yaml"))
  }
}
fn yaml(path: &Path) -> Value {
  serde_saphyr::from_str(&fs::read_to_string(path).unwrap()).unwrap()
}

#[test]
fn tool_probe() {
  let Ok(context) = std::env::var("DOINSTALL_CONTEXT_FILE") else {
    return;
  };
  let context: Value = serde_json::from_slice(&fs::read(context).unwrap()).unwrap();
  if let Ok(mode) = std::env::var("PROBE_SLEEP_CHILD") {
    if mode == "parent" {
      let mut child = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "tool_probe", "--nocapture"])
        .env("PROBE_SLEEP_CHILD", "child")
        .spawn()
        .unwrap();
      std::thread::sleep(std::time::Duration::from_secs(15));
      let _ = child.wait();
    } else {
      std::thread::sleep(std::time::Duration::from_secs(3));
      fs::write(std::env::var("PROBE_CHILD_MARKER").unwrap(), "stranded").unwrap();
    }
    return;
  }
  assert_eq!(context["application-id"], "probe");
  assert!(Path::new(context["build-dir"].as_str().unwrap()).is_dir());
  fs::write(std::env::var("PROBE_CONFIG").unwrap(), context["build-dir"].as_str().unwrap()).unwrap();
  if Path::new(&std::env::var("PROBE_SENTINEL").unwrap()).exists() {
    std::process::exit(17);
  }
}

#[test]
fn validation_does_not_launch_tools_or_create_destination_state() {
  let f = Fixture::new("prepare");
  let result = f.run(&["--validate"]);
  assert!(result.status.success(), "{result:?}");
  assert!(!f.root.exists());
  assert!(!f.managed.exists());
}

#[test]
fn successful_included_tools_capture_owned_configuration_and_reuse_the_package() {
  let f = Fixture::new("after-activate");
  let result = f.run(&[]);
  assert!(result.status.success(), "{result:?}");
  assert!(f.receipt()["managed-files"][f.managed.to_str().unwrap()]["hash"].is_string());
  assert_eq!(yaml(&f.root.join("installer/workflow.yaml"))["state"], "complete");
  assert!(f.run(&[]).status.success());
  assert_eq!(f.receipt()["previous"], Value::Null);
}

#[test]
fn failures_preserve_pending_state_and_retry_the_original_package() {
  for phase in ["prepare", "before-activate", "after-activate"] {
    let f = Fixture::new(phase);
    fs::write(&f.sentinel, "fail").unwrap();
    let result = f.run(&[]);
    assert!(!result.status.success(), "{phase}: {result:?}");
    assert!(f.root.join("installer/pending.yaml").is_file());
    assert_eq!(yaml(&f.root.join("installer/workflow.yaml"))["state"], "failed");
    assert!(!String::from_utf8_lossy(&result.stdout).contains("Installed Probe"));
    fs::remove_file(&f.sentinel).unwrap();
    let retry = f.run(&[]);
    assert!(retry.status.success(), "{phase}: {retry:?}");
    assert!(!f.root.join("installer/pending.yaml").exists());
  }
}

#[test]
fn unowned_and_modified_configuration_are_never_adopted() {
  let f = Fixture::new("prepare");
  fs::write(&f.managed, "user-owned").unwrap();
  let result = f.run(&[]);
  assert!(!result.status.success());
  assert!(!f.root.exists());
  assert_eq!(fs::read_to_string(&f.managed).unwrap(), "user-owned");
  fs::remove_file(&f.managed).unwrap();
  assert!(f.run(&[]).status.success());
  let receipt = fs::read(f.root.join("installer/installation.yaml")).unwrap();
  fs::write(&f.managed, "manual edit").unwrap();
  f.configure(|v| v["application"]["version"] = json!("2.0"));
  assert!(!f.run(&[]).status.success());
  assert_eq!(fs::read(f.root.join("installer/installation.yaml")).unwrap(), receipt);
  assert_eq!(fs::read_to_string(&f.managed).unwrap(), "manual edit");
}

#[test]
fn changed_tools_and_mismatched_destination_requirements_fail_before_mutation() {
  let f = Fixture::new("prepare");
  fs::write(f.payload.join(if cfg!(windows) { "probe.exe" } else { "probe" }), "modified").unwrap();
  assert!(!f.run(&[]).status.success());
  assert!(!f.root.exists());
  let f = Fixture::new("prepare");
  f.configure(|v| v["platform"] = json!(if cfg!(windows) { "linux" } else { "windows" }));
  assert!(!f.run(&[]).status.success());
  assert!(!f.root.exists());
}

#[test]
fn cross_platform_builder_carries_linux_executable_modes_without_running_destination_engine() {
  let f = Fixture::new("prepare");
  f.configure(|v| {
    v["platform"] = json!("linux");
    v["architecture"] = json!("x64");
  });
  let engine = f._temp.path().join("linux-installer");
  let mut header = [0u8; 64];
  header[..4].copy_from_slice(b"\x7fELF");
  header[4] = 2;
  header[5] = 1;
  header[18] = 62;
  fs::write(&engine, header).unwrap();
  let output = f._temp.path().join("package");
  let result = Command::new(env!("CARGO_BIN_EXE_doinstall-builder"))
    .args(["--installer"])
    .arg(&engine)
    .arg("--config")
    .arg(&f.config)
    .arg("--output")
    .arg(&output)
    .output()
    .unwrap();
  assert!(result.status.success(), "{result:?}");
  assert!(output.join("probe-setup-v1.0").is_file());
  assert!(!output.join("installer.exe").exists());
  #[cfg(windows)]
  {
    let mut zip = zip::ZipArchive::new(fs::File::open(output.join("payload.zip")).unwrap()).unwrap();
    assert_eq!(zip.by_name("probe.exe").unwrap().unix_mode().unwrap() & 0o777, 0o755);
  }
  assert!(!f.root.exists());
}

#[test]
fn included_tool_timeout_stops_descendants_and_retains_retry_state() {
  let f = Fixture::new("prepare");
  let marker = f._temp.path().join("child-marker");
  f.configure(|v| {
    v["runtime"]["lifecycle"]["prepare"][0]["timeout-seconds"] = json!(1);
    v["runtime"]["lifecycle"]["prepare"][0]["environment"]["PROBE_SLEEP_CHILD"] = json!("parent");
    v["runtime"]["lifecycle"]["prepare"][0]["environment"]["PROBE_CHILD_MARKER"] = json!(marker);
  });
  let result = f.run(&[]);
  assert!(!result.status.success(), "{result:?}");
  assert!(String::from_utf8_lossy(&result.stderr).contains("exit code 124"));
  std::thread::sleep(std::time::Duration::from_secs(3));
  assert!(!marker.exists());
  assert!(f.root.join("installer/pending.yaml").exists());
  assert_eq!(yaml(&f.root.join("installer/workflow.yaml"))["state"], "failed");
}

#[test]
fn unknown_tool_templates_and_changed_retry_inputs_fail_before_destination_writes() {
  let f = Fixture::new("prepare");
  f.configure(|v| v["runtime"]["lifecycle"]["prepare"][0]["arguments"] = json!(["${unknown-input}"]));
  assert!(!f.run(&["--validate"]).status.success());
  assert!(!f.root.exists());
  let f = Fixture::new("prepare");
  fs::write(&f.sentinel, "fail").unwrap();
  assert!(!f.run(&[]).status.success());
  let before = fs::read(f.root.join("installer/pending.yaml")).unwrap();
  let result = f.run(&["--set", "prune-old-versions=true"]);
  assert!(!result.status.success());
  assert_eq!(fs::read(f.root.join("installer/pending.yaml")).unwrap(), before);
}

#[test]
fn repeated_named_linux_packages_reuse_target_modes_and_keep_empty_directories() {
  let f = Fixture::new("prepare");
  fs::create_dir(f.payload.join("empty")).unwrap();
  f.configure(|v| {
    v["platform"] = json!("linux");
    v["architecture"] = json!("x64");
    v["application"]["build"] = json!({"stamp":"26286-1200"});
  });
  let engine = f._temp.path().join("linux-installer");
  let mut header = [0u8; 64];
  header[..4].copy_from_slice(b"\x7fELF");
  header[4] = 2;
  header[5] = 1;
  header[18] = 62;
  fs::write(&engine, header).unwrap();
  let parent = f._temp.path().join("packages");
  for _ in 0..2 {
    let result = Command::new(env!("CARGO_BIN_EXE_doinstall-builder"))
      .arg("--installer")
      .arg(&engine)
      .arg("--config")
      .arg(&f.config)
      .arg("--output-parent")
      .arg(&parent)
      .output()
      .unwrap();
    assert!(result.status.success(), "{result:?}");
  }
  #[cfg(windows)]
  {
    let package = fs::read_dir(&parent).unwrap().next().unwrap().unwrap().path();
    let mut zip = zip::ZipArchive::new(fs::File::open(package.join("payload.zip")).unwrap()).unwrap();
    assert!(zip.by_name("empty/").unwrap().is_dir());
  }
}
