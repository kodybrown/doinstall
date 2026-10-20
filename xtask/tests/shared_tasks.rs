//! Optional installed-dotask acceptance in an independent disposable project.
//! No task in this flow invokes Git/GitHub or modifies an active installation.
use std::{
  fs,
  path::Path,
  process::{Command, Output},
};
fn copy(source: &Path, destination: &Path) {
  fs::create_dir_all(destination).unwrap();
  for item in fs::read_dir(source).unwrap() {
    let item = item.unwrap();
    let target = destination.join(item.file_name());
    if item.file_type().unwrap().is_dir() {
      copy(&item.path(), &target);
    } else {
      fs::copy(item.path(), target).unwrap();
    }
  }
}
fn success(output: Output) -> Output {
  assert!(
    output.status.success(),
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  output
}
#[test]
fn reusable_tasks_build_run_test_and_explicitly_version_without_project_configuration() {
  if Command::new("dotask").arg("--version").output().is_err() {
    eprintln!("Optional shared-task acceptance skipped: dotask is not installed");
    return;
  }
  let root = Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap();
  let temporary = tempfile::tempdir().unwrap();
  let fixture = temporary.path().join("project");
  fs::create_dir(&fixture).unwrap();
  copy(&root.join(".tasks/_/rust"), &fixture.join(".tasks/_/rust"));
  fs::create_dir(fixture.join("src")).unwrap();
  fs::create_dir(fixture.join(".cargo")).unwrap();
  fs::write(
    fixture.join(".cargo/config.toml"),
    format!(
      "[build]\ntarget-dir = {:?}\n",
      temporary.path().join("target").to_string_lossy().replace('\\', "/")
    ),
  )
  .unwrap();
  let manifest = fixture.join("Cargo.toml");
  let lock = fixture.join("Cargo.lock");
  let source = fixture.join("src/main.rs");
  fs::write(
    &manifest,
    "[package]\nname = \"task-fixture\"\nversion = \"1.2.3\"\nedition = \"2024\"\n",
  )
  .unwrap();
  let program = "fn main(){ let args: Vec<_> = std::env::args().skip(1).collect(); if args == [\"fail\"] { std::process::exit(17); } println!(\"{}\", args.join(\"|\")); }\n#[test] fn example() { assert_eq!(2 + 2, 4); }\n";
  fs::write(&source, program).unwrap();
  success(
    Command::new("cargo")
      .current_dir(&fixture)
      .args(["generate-lockfile", "--offline"])
      .output()
      .unwrap(),
  );
  let original = fs::read(&manifest).unwrap();
  let locked = fs::read(&lock).unwrap();
  let invoke = |task: &str, args: &[&str]| {
    Command::new("dotask")
      .current_dir(&fixture)
      .arg(task)
      .args(args)
      .output()
      .unwrap()
  };
  success(invoke("_/rust/build", &[]));
  assert_ne!(fs::read_to_string(&source).unwrap(), program, "Build must apply formatting");
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  let output = success(invoke(
    "_/rust/run",
    &[
      "--args",
      r#"["two words","", "quote\"slash\\", "\u65e5\u672c\u8a9e"]"#,
    ],
  ));
  let stdout = String::from_utf8_lossy(&output.stdout);
  assert!(
    stdout.contains("two words||quote\"slash\\|\u{65e5}\u{672c}\u{8a9e}"),
    "{stdout}"
  );
  assert_eq!(
    stdout.matches("Private-content check passed:").count(),
    1,
    "Run builds/scans once"
  );
  assert_eq!(invoke("_/rust/run", &["--args", r#"["fail"]"#]).status.code(), Some(17));
  assert!(!invoke("_/rust/run", &["--args", r#"[1]"#]).status.success());
  success(invoke("_/rust/test", &[]));
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  // Explicit versioning changes only version files. No selector means patch;
  // previews and conflicting selectors leave both files intact.
  success(invoke("_/rust/update-version", &["--dry-run"]));
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  assert!(!invoke("_/rust/update-version", &["--increment", "--increment-major"])
    .status
    .success());
  for (args, version) in [
    (vec![], "1.2.4"),
    (vec!["--increment"], "1.2.5"),
    (vec!["--increment-minor"], "1.3.0"),
    (vec!["--increment-major"], "2.0.0"),
  ] {
    success(invoke("_/rust/update-version", &args));
    for path in [&manifest, &lock] {
      assert!(fs::read_to_string(path).unwrap().contains(&format!("version = \"{version}\"")));
    }
  }
  fs::write(&source, "fn main() { let unused = 1; }\n").unwrap();
  let output = invoke("_/rust/run", &[]);
  assert!(!output.status.success());
  assert!(
    !String::from_utf8_lossy(&output.stdout).contains("Private-content check passed:"),
    "Clippy must stop before build/run"
  );
  fs::write(&source, "fn main() { println!(\"/home/private/build-machine\"); }\n").unwrap();
  let output = invoke("_/rust/test", &[]);
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Private content"));
  assert!(
    !String::from_utf8_lossy(&output.stdout).contains("test result:"),
    "Scan failure must stop before tests"
  );
}
