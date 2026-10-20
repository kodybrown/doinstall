//! Optional installed-dotask acceptance. Fake Git/GitHub executables make tag
//! creation and publication impossible, including while exercising --push.
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
fn reusable_tasks_work_without_project_configuration_or_real_publication() {
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
  fs::write(
    fixture.join("Cargo.toml"),
    "[package]\nname = \"task-fixture\"\nversion = \"1.2.3\"\nedition = \"2024\"\n",
  )
  .unwrap();
  fs::write(fixture.join("src/main.rs"), "fn main() { println!(\"fixture\"); }\n").unwrap();
  success(
    Command::new("cargo")
      .current_dir(&fixture)
      .args([
        "metadata",
        "--offline",
        "--format-version",
        "1",
        "--no-deps",
      ])
      .output()
      .unwrap(),
  );
  success(
    Command::new("cargo")
      .current_dir(&fixture)
      .args(["generate-lockfile", "--offline"])
      .output()
      .unwrap(),
  );
  let tools = temporary.path().join("tools");
  fs::create_dir(&tools).unwrap();
  let mock = tools.join("mock.rs");
  fs::write(&mock,r#"
use std::{env,fs,io::Write,path::Path};
fn main() {
  let tool = env::args().next().unwrap(); let tool = Path::new(&tool).file_stem().unwrap().to_str().unwrap();
  let args: Vec<_> = env::args().skip(1).collect(); let log = env::var("TASK_TEST_LOG").unwrap();
  writeln!(fs::OpenOptions::new().create(true).append(true).open(&log).unwrap(),"{}\x1f{}",tool,args.join("\x1f")).unwrap();
  let case = env::var("TASK_TEST_CASE").unwrap_or_default();
  if args == ["--version"] { println!("version 1.0"); return; }
  if tool == "gh" {
    assert_eq!(&args[..2],["run","list"]);
    let query = args.last().unwrap(); assert!(query.contains(&format!(".headSha == \"{}\"","a".repeat(40))));
    assert!(query.contains(".status == \"completed\"")); assert!(query.contains(".conclusion == \"success\""));
    std::process::exit(if case == "ci-failure" {1}else{0});
  }
  match args[0].as_str() {
    "diff" => std::process::exit(if case == "dirty" || case == "staged" && args.contains(&"--cached".into()) {1}else{0}),
    "log" => print!("{}","a".repeat(40)),
    "fetch" => {assert_eq!(args,["fetch","--no-tags","origin","refs/heads/main:refs/remotes/origin/main"]);},
    "merge-base" => {
      let count = fs::read_to_string(log).unwrap().lines().filter(|v|v.starts_with("git\x1fmerge-base\x1f")).count();
      std::process::exit(if case == format!("mismatch-{count}") {1}else{0});
    },
    "tag" => std::process::exit(if case == "existing-tag" {1}else{0}),
    "push" => std::process::exit(if case == "push-failure" {1}else{0}),
    "hook-tag" => {},
    "hook-package" => std::process::exit(if case == "package-failure" {1}else{0}),
    _ => panic!("unexpected command: {args:?}"),
  }
}
"#).unwrap();
  let git = tools.join(if cfg!(windows) { "git.exe" } else { "git" });
  success(Command::new("rustc").arg(&mock).arg("-o").arg(&git).output().unwrap());
  fs::copy(&git, tools.join(if cfg!(windows) { "gh.exe" } else { "gh" })).unwrap();
  let path =
    std::env::join_paths(std::iter::once(tools).chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())))
      .unwrap();
  let log = temporary.path().join("commands.log");
  let invoke = |task: &str, args: &[&str], case: &str| {
    Command::new("dotask")
      .current_dir(&fixture)
      .arg(task)
      .args(args)
      .env("PATH", &path)
      .env("TASK_TEST_LOG", &log)
      .env("TASK_TEST_CASE", case)
      .output()
      .unwrap()
  };
  for case in [
    "dirty",
    "staged",
    "ci-failure",
    "mismatch-1",
    "mismatch-2",
    "mismatch-3",
    "mismatch-4",
    "mismatch-5",
    "mismatch-6",
  ] {
    fs::write(&log, "").unwrap();
    let output = invoke("_/rust/tag-release", &["--push"], case);
    assert!(!output.status.success(), "Guard should fail: {case}");
    assert!(
      log.exists(),
      "Guard {case} did not reach mock tools: {}{}",
      String::from_utf8_lossy(&output.stdout),
      String::from_utf8_lossy(&output.stderr)
    );
    let log = fs::read_to_string(&log).unwrap();
    assert!(!log.contains("git\x1ftag\x1f") && !log.contains("git\x1fpush\x1f"));
  }
  fs::write(&log, "").unwrap();
  success(invoke("_/rust/tag-release", &["--create=false", "--push"], ""));
  assert!(!fs::read_to_string(&log).unwrap().contains("git\x1ftag\x1f"));
  fs::write(&log, "").unwrap();
  success(invoke("_/rust/tag-release", &["--push"], ""));
  let commands = fs::read_to_string(&log).unwrap();
  assert!(commands.contains(&format!(
    "git\x1ftag\x1f-a\x1fv1.2.3\x1f{}\x1f-m\x1fRelease v1.2.3",
    "a".repeat(40)
  )));
  assert!(commands.ends_with("git\x1fpush\x1forigin\x1frefs/tags/v1.2.3\n"));
  for case in ["existing-tag", "push-failure"] {
    fs::write(&log, "").unwrap();
    assert!(!invoke("_/rust/tag-release", &["--push"], case).status.success());
    let commands = fs::read_to_string(&log).unwrap();
    assert!(!commands.contains("\x1f--force") && !commands.contains("\x1f-d\x1f"));
    if case == "existing-tag" {
      assert!(!commands.contains("git\x1fpush\x1f"));
    }
  }
  let manifest = fixture.join("Cargo.toml");
  let lock = fixture.join("Cargo.lock");
  let original = fs::read(&manifest).unwrap();
  let locked = fs::read(&lock).unwrap();
  success(invoke("_/rust/update-version", &["--increment", "--dry-run"], ""));
  assert_eq!(fs::read(&manifest).unwrap(), original);
  assert_eq!(fs::read(&lock).unwrap(), locked);
  assert!(!invoke("_/rust/update-version", &["--increment", "--increment-major"], "")
    .status
    .success());
  success(invoke("_/rust/update-version", &[], ""));
  assert_eq!(fs::read(&manifest).unwrap(), original);
  for (option, version) in [
    ("--increment", "1.2.4"),
    ("--increment-minor", "1.3.0"),
    ("--increment-major", "2.0.0"),
  ] {
    success(invoke("_/rust/update-version", &[option], ""));
    assert!(fs::read_to_string(&manifest)
      .unwrap()
      .contains(&format!("version = \"{version}\"")));
    assert!(fs::read_to_string(&lock).unwrap().contains(&format!("version = \"{version}\"")));
  }
  success(invoke("_/rust/build", &[], ""));
  fs::write(
    fixture.join("src/main.rs"),
    "fn main() { println!(\"/home/private/build-machine\"); }\n",
  )
  .unwrap();
  let output = invoke("_/rust/build", &[], "");
  assert!(!output.status.success());
  assert!(String::from_utf8_lossy(&output.stderr).contains("Private content"));

  // Project hooks keep payload policy out of the reusable pipeline. Verify that
  // packaging failure prevents the final tagging hook; no real tags are made.
  let command = |name: &str| serde_json::json!({"exec":{"executable":git.to_string_lossy(),"arguments":[name]}});
  fs::write(fixture.join(".tasks/tag-release.task"),serde_json::to_string(&serde_json::json!({"description":"Mock policy hook","options":[{"name":"create","type":"bool","default":true},{"name":"push","type":"bool","default":false}],"steps":[command("hook-tag")]})).unwrap()).unwrap();
  fs::write(
    fixture.join(".tasks/create-installer.task"),
    serde_json::to_string(&serde_json::json!({"description":"Mock packaging hook","steps":[command("hook-package")]}))
      .unwrap(),
  )
  .unwrap();
  fs::write(fixture.join("src/main.rs"), "fn main() {\n    println!(\"fixture\");\n}\n").unwrap();
  fs::write(&log, "").unwrap();
  let output = invoke("_/rust/publish", &["--push"], "package-failure");
  assert!(!output.status.success(), "Packaging failure should stop publish");
  let commands = fs::read_to_string(&log).unwrap();
  assert!(
    commands.contains("git\x1fhook-package"),
    "{}{}",
    String::from_utf8_lossy(&output.stdout),
    String::from_utf8_lossy(&output.stderr)
  );
  assert_eq!(commands.matches("git\x1fhook-tag").count(), 1);
  fs::write(&log, "").unwrap();
  success(invoke("_/rust/publish", &[], ""));
  let commands = fs::read_to_string(&log).unwrap();
  assert_eq!(commands.matches("git\x1fhook-tag").count(), 1);
  assert!(commands.ends_with("git\x1fhook-package\n"));
  fs::write(&log, "").unwrap();
  success(invoke("_/rust/publish", &["--push"], ""));
  let commands = fs::read_to_string(&log).unwrap();
  assert_eq!(commands.matches("git\x1fhook-tag").count(), 2);
  assert!(commands.ends_with("git\x1fhook-tag\n"));
}
