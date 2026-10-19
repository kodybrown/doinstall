use serde_json::{json, Value};
use std::{
  fs,
  path::PathBuf,
  process::{Command, Output},
};

struct Fixture {
  temp: tempfile::TempDir,
  config: PathBuf,
  root: PathBuf,
  settings: PathBuf,
}
impl Fixture {
  fn new() -> Self {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("application");
    let settings = temp.path().join("editable/settings.yaml");
    let payload = temp.path().join("payload");
    fs::create_dir(&payload).unwrap();
    let executable = if cfg!(windows) { "probe.exe" } else { "probe" };
    fs::copy(env!("CARGO_BIN_EXE_doinstall"), payload.join(executable)).unwrap();
    fs::write(
      payload.join("defaults.yaml"),
      "entries:\n  - filename: original\n    destination: original\n    enabled: true\n",
    )
    .unwrap();
    let config = temp.path().join("installer.yaml");
    let package = json!({"schema":1,"application":{"id":"probe","name":"Probe","version":"1.0"},
      "platform":if cfg!(windows){"windows"}else if cfg!(target_os="macos"){"macos"}else{"linux"},
      "architecture":if cfg!(target_arch="aarch64"){"arm64"}else{"x64"},"payload":"payload",
      "runtime":{"commands":[{"name":"probe","executable":executable}],
        "values":{"install-dir":root,"add-to-path":false},
        "inputs":{"filename":{"type":"string","expand":false,"required":true,"default":"${HOST}-${date:yyyyMMdd}.txt"},
          "destination":{"type":"path","default":"${home}/Backups"}},
        "settings-files":[{"source":"defaults.yaml","destination":settings,"format":"yaml","if-exists":"preserve",
          "set":{"/entries/0/filename":"filename","/entries/0/destination":"destination"}}]}});
    fs::write(&config, serde_saphyr::to_string(&package).unwrap()).unwrap();
    Self {
      temp,
      config,
      root,
      settings,
    }
  }
  fn run(&self, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_doinstall"))
      .arg("--config")
      .arg(&self.config)
      .arg("--non-interactive")
      .args(args)
      .output()
      .unwrap()
  }
  fn good(&self, args: &[&str]) {
    let output = self.run(args);
    assert!(
      output.status.success(),
      "stdout: {}\nstderr: {}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
  }
  fn change(&self, update: impl FnOnce(&mut Value)) {
    let mut package: Value = serde_saphyr::from_str(&fs::read_to_string(&self.config).unwrap()).unwrap();
    update(&mut package);
    fs::write(&self.config, serde_saphyr::to_string(&package).unwrap()).unwrap();
  }
  fn uninstall(&self, remove: bool) {
    let output = Command::new(self.root.join(if cfg!(windows) {
      "installer/installer.exe"
    } else {
      "installer/installer"
    }))
    .args([
      "uninstall",
      "--non-interactive",
      if remove {
        "--remove-settings"
      } else {
        "--leave-settings"
      },
    ])
    .output()
    .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
  }
}

#[test]
fn initialize_literal_templates_and_custom_values_then_preserve_edited_settings_on_upgrade() {
  let fixture = Fixture::new();
  let filename = "${HOST}-quoted-\"value\"-${date:yyyyMMdd}.txt";
  fixture.good(&["--set", &format!("filename={filename}")]);
  let settings: Value = serde_saphyr::from_str(&fs::read_to_string(&fixture.settings).unwrap()).unwrap();
  assert_eq!(settings["entries"][0]["filename"], filename);
  assert!(settings["entries"][0]["destination"].as_str().unwrap().ends_with("/Backups"));
  let customized = b"# user comments and formatting\r\nentries: []\r\n";
  fs::write(&fixture.settings, customized).unwrap();
  fixture.change(|package| package["application"]["version"] = "2.0".into());
  fixture.good(&["--set", "filename=different"]);
  assert_eq!(fs::read(&fixture.settings).unwrap(), customized);
  fixture.uninstall(false);
  assert_eq!(fs::read(&fixture.settings).unwrap(), customized);
}

#[test]
fn preexisting_settings_are_preserved_without_claiming_uninstall_ownership() {
  let fixture = Fixture::new();
  fs::create_dir_all(fixture.settings.parent().unwrap()).unwrap();
  fs::write(&fixture.settings, b"\xef\xbb\xbf# existing\r\noriginal: true\r\n").unwrap();
  let original = fs::read(&fixture.settings).unwrap();
  fixture.good(&[]);
  fixture.uninstall(true);
  assert_eq!(fs::read(&fixture.settings).unwrap(), original);
}

#[test]
fn only_created_settings_can_be_removed_explicitly_and_backups_are_untouched() {
  let fixture = Fixture::new();
  let backup = fixture.temp.path().join("backups/user.txt");
  fs::create_dir_all(backup.parent().unwrap()).unwrap();
  fs::write(&backup, "backup content").unwrap();
  fixture.good(&[]);
  fixture.uninstall(true);
  assert!(!fixture.settings.exists());
  assert_eq!(fs::read(backup).unwrap(), b"backup content");
}

#[test]
fn validate_is_read_only_and_bad_pointer_or_missing_defaults_fail_before_installation() {
  let fixture = Fixture::new();
  fixture.good(&["--validate"]);
  assert!(!fixture.root.exists());
  assert!(!fixture.settings.parent().unwrap().exists());
  fixture.change(|package| package["runtime"]["settings-files"][0]["set"] = json!({"/missing":"filename"}));
  assert!(!fixture.run(&[]).status.success());
  assert!(!fixture.root.exists());
  fixture.change(|package| package["runtime"]["settings-files"][0]["source"] = "missing.yaml".into());
  assert!(!fixture.run(&["--validate"]).status.success());
  assert!(!fixture.root.exists());
}

#[test]
fn unknown_inputs_types_paths_and_policies_are_rejected() {
  for change in 0..6 {
    let fixture = Fixture::new();
    fixture.change(|package| match change {
      0 => package["runtime"]["settings-files"][0]["set"] = json!({"/entries/0/filename":"unknown"}),
      1 => package["runtime"]["settings-files"][0]["source"] = "../outside.yaml".into(),
      2 => package["runtime"]["settings-files"][0]["destination"] = json!(fixture.root.join("settings.yaml")),
      3 => package["runtime"]["settings-files"][0]["if-exists"] = "overwrite".into(),
      4 => package["runtime"]["inputs"]["filename"]["type"] = "path".into(),
      _ => package["runtime"]["settings-files"][0]["set"] = json!({"/entries/0/enabled":"filename"}),
    });
    assert!(!fixture.run(&[]).status.success(), "case {change}");
    assert!(!fixture.root.exists());
  }
}

#[test]
fn json_settings_and_duplicate_destinations_are_supported_and_checked() {
  let fixture = Fixture::new();
  fs::write(fixture.temp.path().join("payload/defaults.json"), r#"{"name":"original"}"#).unwrap();
  fixture.change(|package| {
    package["runtime"]["settings-files"][0] = json!({"source":"defaults.json", "destination":fixture.settings,
    "format":"json","if-exists":"preserve","set":{"/name":"filename"}})
  });
  fixture.good(&[]);
  let content: Value = serde_json::from_slice(&fs::read(&fixture.settings).unwrap()).unwrap();
  assert_eq!(content["name"], "${HOST}-${date:yyyyMMdd}.txt");
  fixture.change(|package| {
    let duplicate = package["runtime"]["settings-files"][0].clone();
    package["runtime"]["settings-files"].as_array_mut().unwrap().push(duplicate);
  });
  assert!(!fixture.run(&["--validate"]).status.success());
}

#[test]
fn plain_seed_preserves_default_comments_bom_and_line_endings() {
  let fixture = Fixture::new();
  let source = b"\xef\xbb\xbf# useful explanation\r\nvalue: preserved\r\n";
  fs::write(fixture.temp.path().join("payload/defaults.yaml"), source).unwrap();
  fixture.change(|package| package["runtime"]["settings-files"][0]["set"] = json!({}));
  fixture.good(&[]);
  assert_eq!(fs::read(&fixture.settings).unwrap(), source);
}

#[test]
fn pending_retry_recovers_recorded_settings_ownership_without_duplicates() {
  let fixture = Fixture::new();
  fixture.good(&[]);
  let management = fixture.root.join("installer");
  let receipt_path = management.join("installation.yaml");
  let receipt: Value = serde_saphyr::from_str(&fs::read_to_string(&receipt_path).unwrap()).unwrap();
  fs::write(management.join("pending.yaml"), serde_saphyr::to_string(&receipt).unwrap()).unwrap();
  let mut earlier = receipt;
  earlier["settings"] = json!([]);
  fs::write(&receipt_path, serde_saphyr::to_string(&earlier).unwrap()).unwrap();
  fixture.good(&[]);
  let receipt: Value = serde_saphyr::from_str(&fs::read_to_string(&receipt_path).unwrap()).unwrap();
  assert_eq!(receipt["settings"].as_array().unwrap().len(), 1);
  fixture.uninstall(true);
  assert!(!fixture.settings.exists());
}
