//! Small native operations that need structured data or private process I/O.
//! Package/service orchestration stays in the reusable YAML recipe.
#[path = "../tool_version.rs"]
mod tool_version;
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::{
  collections::{BTreeMap, BTreeSet},
  fs::{self, File, OpenOptions},
  io::{Read, Write},
  net::{SocketAddr, TcpStream},
  path::{Component, Path, PathBuf},
  process::{Command, Stdio},
  time::{Duration, Instant},
};
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Stack {
  schema: u32,
  id: String,
  domain: String,
  port: u16,
  executable: String,
  #[serde(default)]
  arguments: Vec<String>,
  #[serde(default)]
  environment: BTreeMap<String, String>,
  database: Database,
  redis: Redis,
  #[serde(default)]
  tls: Tls,
  #[serde(default)]
  frontend: Option<String>,
  #[serde(default)]
  migrations: Option<Migration>,
  #[serde(default = "default_health")]
  health_path: String,
  #[serde(default = "default_status")]
  health_status: u16,
}
fn default_health() -> String {
  "/health".into()
}
fn default_status() -> u16 {
  200
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Database {
  role: String,
  /// Environment variable -> database name, allowing multiple contexts per app.
  connections: BTreeMap<String, String>,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
struct Redis {
  port: u16,
  host_environment: String,
  port_environment: String,
  password_environment: String,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Tls {
  #[serde(default)]
  enabled: bool,
  #[serde(default)]
  email: String,
}
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Migration {
  executable: String,
  #[serde(default)]
  arguments: Vec<String>,
  #[serde(default)]
  environment: BTreeMap<String, String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "kebab-case")]
struct ContextFile {
  schema: u32,
  application_id: String,
  install_dir: PathBuf,
  build_dir: PathBuf,
  previous_build_dir: Option<PathBuf>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Credentials {
  schema: u32,
  application: String,
  database: String,
  redis: String,
}
fn main() {
  if tool_version::print(&std::env::args().skip(1).collect::<Vec<_>>(), "doinstall-server") {
    return;
  }
  if let Err(error) = run() {
    // Do not include SQL, credentials, child stderr, or private file contents.
    eprintln!("doinstall-server: {error:#}");
    std::process::exit(1);
  }
}
fn run() -> Result<()> {
  let args: Vec<_> = std::env::args().skip(1).collect();
  if args == ["--help"] || args.is_empty() {
    println!("doinstall-server ACTION --config FILE\nActions: validate, describe, plan, account, credentials, postgres, backup, migrate, readiness, proxy-readiness, cleanup\nvalidate/describe are build-machine operations. Other actions require DOINSTALL_CONTEXT_FILE and Linux root.");
    return Ok(());
  }
  ensure!(args.len() == 3 && args[1] == "--config", "Use ACTION --config FILE.");
  let text = fs::read_to_string(&args[2]).context("Read stack configuration")?;
  let config: Stack = serde_saphyr::from_str(&text).context("Invalid stack configuration")?;
  validate(&config)?;
  if args[0] == "validate" {
    return Ok(());
  }
  if args[0] == "describe" {
    println!("{}", json!({"managed-files": managed_files(&config)}));
    return Ok(());
  }
  destination()?;
  let context: ContextFile = serde_json::from_slice(&fs::read(
    std::env::var("DOINSTALL_CONTEXT_FILE").context("Installer context is required")?,
  )?)?;
  ensure!(
    context.schema == 1 && context.application_id == config.id,
    "Stack and installer identity differ."
  );
  safe(&context.install_dir)?;
  safe(&context.build_dir)?;
  ensure!(
    context.build_dir.starts_with(context.install_dir.join("app")),
    "Build directory is outside the installation."
  );
  ensure!(
    context.build_dir.join(&config.executable).is_file(),
    "Application executable is missing."
  );
  let plan_path = PathBuf::from(format!("/run/doinstall-{}/plan.json", config.id));
  ensure!(
    std::env::var_os("DOINSTALL_SERVER_PLAN").is_some_and(|p| p == plan_path.as_os_str()),
    "Unexpected plan file location."
  );
  match args[0].as_str() {
    "plan" => plan(&config, &context, &plan_path),
    "account" => account(&config),
    "credentials" => credentials(&config).map(|_| ()),
    "postgres" => postgres(&config),
    "backup" => backup(&config, &context),
    "migrate" => migrate(&config, &context),
    "readiness" => readiness(&config),
    "proxy-readiness" => proxy_readiness(&config),
    "cleanup" => {
      if plan_path.exists() {
        safe(&plan_path)?;
        fs::remove_file(plan_path)?;
      }
      Ok(())
    }
    _ => bail!("Unknown action."),
  }
}
fn token(value: &str, extra: &str, max: usize) -> bool {
  !value.is_empty()
    && value.len() <= max
    && value
      .bytes()
      .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || extra.as_bytes().contains(&c))
}
fn env_key(key: &str) -> bool {
  !key.is_empty()
    && key
      .bytes()
      .enumerate()
      .all(|(i, c)| c.is_ascii_alphabetic() || c == b'_' || (i > 0 && c.is_ascii_digit()))
}
fn relative(value: &str) -> bool {
  !value.is_empty()
    && value.split('/').all(|p| {
      !p.is_empty() && p != "." && p != ".." && p.bytes().all(|c| c.is_ascii_alphanumeric() || b"-_.".contains(&c))
    })
}
fn validate(c: &Stack) -> Result<()> {
  ensure!(c.schema == 1, "Unsupported stack schema.");
  ensure!(
    token(&c.id, "-", 28) && c.id.as_bytes()[0].is_ascii_lowercase(),
    "Invalid application id."
  );
  ensure!(
    token(&c.domain, ".-", 253)
      && c
        .domain
        .split('.')
        .all(|p| !p.is_empty() && !p.starts_with('-') && !p.ends_with('-') && p.len() <= 63),
    "Invalid domain."
  );
  ensure!(
    c.port >= 1024 && c.redis.port >= 1024 && c.port != c.redis.port,
    "Use distinct unprivileged API/Redis ports."
  );
  ensure!(
    relative(&c.executable) && c.executable.starts_with("api/"),
    "Executable must be a safe api/ payload path."
  );
  ensure!(
    token(&c.database.role, "_", 48) && c.database.role.as_bytes()[0].is_ascii_lowercase(),
    "Invalid database role."
  );
  ensure!(
    !c.database.connections.is_empty(),
    "At least one database connection is required."
  );
  for (key, db) in &c.database.connections {
    ensure!(
      env_key(key) && token(db, "_", 48) && db.as_bytes()[0].is_ascii_lowercase(),
      "Invalid database mapping."
    );
    ensure!(
      !["postgres", "template0", "template1"].contains(&db.as_str()),
      "System databases cannot be application databases."
    );
  }
  let mut keys: BTreeSet<_> = c.database.connections.keys().collect();
  for key in [
    &c.redis.host_environment,
    &c.redis.port_environment,
    &c.redis.password_environment,
  ] {
    ensure!(
      env_key(key) && keys.insert(key),
      "Invalid or duplicate generated environment key."
    );
  }
  for (key, value) in &c.environment {
    ensure!(
      env_key(key) && !keys.contains(key) && !["DOTNET_ROOT", "ASPNETCORE_URLS"].contains(&key.as_str()),
      "Reserved or invalid environment key."
    );
    ensure!(!value.contains(['\r', '\n', '\0']), "Environment values must fit one line.");
  }
  ensure!(
    !c.tls.enabled
      || (c.tls.email.contains('@') && !c.tls.email.starts_with('-') && !c.tls.email.chars().any(char::is_whitespace)),
    "TLS requires a contact email."
  );
  ensure!(
    c.frontend.as_ref().is_none_or(|p| relative(p) && p.starts_with("web/")),
    "Frontend must be a web/ payload path."
  );
  ensure!(
    c.migrations
      .as_ref()
      .is_none_or(|m| relative(&m.executable) && m.executable.starts_with("api/")),
    "Migration tool must be an api/ payload executable."
  );
  if let Some(migration) = &c.migrations {
    for (key, value) in &migration.environment {
      ensure!(
        env_key(key)
          && !keys.contains(key)
          && !["DOTNET_ROOT", "ASPNETCORE_URLS"].contains(&key.as_str())
          && !value.contains(['\n', '\r', '\0']),
        "Invalid migration environment."
      );
    }
  }
  ensure!(
    c.health_path.starts_with('/') && c.health_path.bytes().all(|b| b.is_ascii_graphic() && b != b'"' && b != b'\\'),
    "Invalid health path."
  );
  ensure!((200..=599).contains(&c.health_status), "Invalid health status.");
  for argument in c.arguments.iter().chain(c.migrations.iter().flat_map(|m| m.arguments.iter())) {
    ensure!(
      !argument.contains(['\n', '\r', '\0']),
      "Arguments cannot contain control separators."
    );
  }
  Ok(())
}
fn managed_files(c: &Stack) -> Vec<String> {
  let id = &c.id;
  vec![
    format!("/etc/{id}/credentials.json"),
    format!("/etc/{id}/api.env"),
    format!("/etc/{id}/redis.conf"),
    format!("/etc/systemd/system/{id}.service"),
    format!("/etc/systemd/system/{id}-redis.service"),
    format!("/etc/nginx/conf.d/{id}.conf"),
    format!("/etc/systemd/system/{id}-certificates.service"),
    format!("/etc/systemd/system/{id}-certificates.timer"),
  ]
}
fn safe(path: &Path) -> Result<()> {
  ensure!(
    path.is_absolute()
      && path
        .to_str()
        .is_some_and(|p| p.bytes().all(|b| b.is_ascii_alphanumeric() || b"/-._".contains(&b))),
    "Unsafe native path."
  );
  let mut current = PathBuf::new();
  for part in path.components() {
    ensure!(!matches!(part, Component::ParentDir), "Parent traversal.");
    current.push(part);
    match fs::symlink_metadata(&current) {
      Ok(meta) => ensure!(!meta.file_type().is_symlink(), "Refusing linked path."),
      Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
      Err(e) => return Err(e.into()),
    }
  }
  Ok(())
}
fn directory(path: &Path, mode: u32) -> Result<()> {
  safe(path)?;
  if !path.exists() {
    #[cfg(unix)]
    {
      use std::os::unix::fs::DirBuilderExt;
      fs::DirBuilder::new().mode(mode).recursive(true).create(path)?;
    }
    #[cfg(not(unix))]
    {
      let _ = mode;
      fs::create_dir_all(path)?;
    }
  }
  ensure!(path.is_dir(), "Expected directory.");
  Ok(())
}
fn publish(path: &Path, bytes: &[u8], mode: u32) -> Result<()> {
  safe(path)?;
  let mut temporary = tempfile::NamedTempFile::new_in(path.parent().context("Missing parent")?)?;
  temporary.write_all(bytes)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    temporary.as_file().set_permissions(fs::Permissions::from_mode(mode))?;
  }
  #[cfg(not(unix))]
  let _ = mode;
  temporary.as_file().sync_all()?;
  temporary.persist(path).map_err(|e| e.error)?;
  Ok(())
}
fn destination() -> Result<()> {
  ensure!(cfg!(target_os = "linux"), "Server operations require Linux.");
  #[cfg(unix)]
  ensure!(unsafe { libc::geteuid() } == 0, "Server operations require root.");
  let os = fs::read_to_string("/etc/os-release")?;
  let fields: BTreeMap<_, _> = os
    .lines()
    .filter_map(|l| l.split_once('='))
    .map(|(k, v)| (k, v.trim_matches('"')))
    .collect();
  ensure!(
    matches!(
      (fields.get("ID").copied(), fields.get("VERSION_ID").copied()),
      (Some("debian"), Some("12")) | (Some("ubuntu"), Some("24.04"))
    ),
    "Supported hosts are Debian 12 and Ubuntu 24.04."
  );
  ensure!(Path::new("/run/systemd/system").is_dir(), "A running systemd host is required.");
  Ok(())
}
fn read_credentials(c: &Stack) -> Result<Option<Credentials>> {
  let path = PathBuf::from(format!("/etc/{}/credentials.json", c.id));
  safe(&path)?;
  if !path.exists() {
    return Ok(None);
  }
  #[cfg(unix)]
  {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let meta = fs::metadata(&path)?;
    ensure!(
      meta.uid() == 0 && meta.permissions().mode() & 0o777 == 0o600,
      "Credentials require root ownership and mode 0600."
    );
  }
  let value: Credentials = serde_json::from_slice(&fs::read(path)?).context("Invalid credentials file")?;
  ensure!(
    value.schema == 1
      && value.application == c.id
      && [&value.database, &value.redis]
        .iter()
        .all(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit())),
    "Credentials identity or format differs."
  );
  Ok(Some(value))
}
fn credentials(c: &Stack) -> Result<Credentials> {
  if let Some(value) = read_credentials(c)? {
    return Ok(value);
  }
  let dir = PathBuf::from(format!("/etc/{}", c.id));
  directory(&dir, 0o755)?;
  let value = Credentials {
    schema: 1,
    application: c.id.clone(),
    database: random()?,
    redis: random()?,
  };
  let path = dir.join("credentials.json");
  let mut temporary = tempfile::NamedTempFile::new_in(&dir)?;
  temporary.write_all(&serde_json::to_vec(&value)?)?;
  #[cfg(unix)]
  {
    use std::os::unix::fs::PermissionsExt;
    temporary.as_file().set_permissions(fs::Permissions::from_mode(0o600))?;
  }
  temporary.as_file().sync_all()?;
  // Concurrent creation must never replace an established password.
  temporary.persist_noclobber(path).map_err(|e| e.error)?;
  Ok(value)
}
fn random() -> Result<String> {
  let mut bytes = [0u8; 32];
  File::open("/dev/urandom")?.read_exact(&mut bytes)?;
  Ok(bytes.iter().map(|b| format!("{b:02x}")).collect())
}
fn systemd_argument(value: &str) -> String {
  format!(
    "\"{}\"",
    value
      .replace('\\', "\\\\")
      .replace('"', "\\\"")
      .replace('%', "%%")
      .replace('$', "$$")
  )
}
fn environment(c: &Stack, context: &ContextFile, secret: &Credentials) -> BTreeMap<String, String> {
  let mut env = c.environment.clone();
  env.insert("DOTNET_ROOT".into(), context.build_dir.join("runtime").display().to_string());
  env.insert("ASPNETCORE_URLS".into(), format!("http://127.0.0.1:{}", c.port));
  for (key, db) in &c.database.connections {
    env.insert(
      key.clone(),
      format!(
        "Host=127.0.0.1;Port=5432;Database={db};Username={};Password={};",
        c.database.role, secret.database
      ),
    );
  }
  env.insert(c.redis.host_environment.clone(), "127.0.0.1".into());
  env.insert(c.redis.port_environment.clone(), c.redis.port.to_string());
  env.insert(c.redis.password_environment.clone(), secret.redis.clone());
  env
}
fn plan(c: &Stack, context: &ContextFile, path: &Path) -> Result<()> {
  directory(path.parent().unwrap(), 0o700)?;
  let secret = read_credentials(c)?;
  let env_content = secret
    .as_ref()
    .map(|s| {
      environment(c, context, s)
        .iter()
        .map(|(k, v)| format!("{k}=\"{}\"\n", v.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect::<String>()
    })
    .unwrap_or_default();
  let runtime = context.build_dir.join("runtime/dotnet");
  safe(&runtime)?;
  ensure!(runtime.is_file(), "Bundled .NET runtime is missing.");
  if let Some(frontend) = &c.frontend {
    ensure!(
      context.build_dir.join(frontend).join("index.html").is_file(),
      "Frontend index.html is missing."
    );
  }
  let os = fs::read_to_string("/etc/os-release")?;
  let ubuntu = os.lines().any(|l| l == "ID=ubuntu");
  let exec = std::iter::once(systemd_argument(&context.build_dir.join(&c.executable).display().to_string()))
    .chain(c.arguments.iter().map(|a| systemd_argument(a)))
    .collect::<Vec<_>>()
    .join(" ");
  let id = &c.id;
  let cert = PathBuf::from(format!("/etc/letsencrypt/live/{id}/fullchain.pem"));
  let tls_ready = c.tls.enabled && cert.is_file();
  let acme = format!("/var/lib/{id}/acme");
  let location = c.frontend.as_ref().map(|p|format!("location / {{ root {}/{}; try_files $uri $uri/ /index.html; }}\nlocation /api/ {{ proxy_pass http://127.0.0.1:{}; include /etc/nginx/proxy_params; }}",context.build_dir.display(),p,c.port)).unwrap_or_else(||format!("location / {{ proxy_pass http://127.0.0.1:{}; include /etc/nginx/proxy_params; }}",c.port));
  let value = json!({"id":id,"service":format!("{id}.service"),"redis_service":format!("{id}-redis.service"),"certificate_timer":format!("{id}-certificates.timer"),
    "config_dir":format!("/etc/{id}"),"state_dir":format!("/var/lib/{id}"),"log_dir":format!("/var/log/{id}"),
    "acme_dir":acme,"backup_dir":format!("/var/backups/{id}"),"env_path":format!("/etc/{id}/api.env"),
    "redis_path":format!("/etc/{id}/redis.conf"),"unit_path":format!("/etc/systemd/system/{id}.service"),
    "redis_unit_path":format!("/etc/systemd/system/{id}-redis.service"),"nginx_path":format!("/etc/nginx/conf.d/{id}.conf"),
    "renew_unit_path":format!("/etc/systemd/system/{id}-certificates.service"),"renew_timer_path":format!("/etc/systemd/system/{id}-certificates.timer"),
    "icu_package":if ubuntu {"libicu74"} else {"libicu72"},"ssl_package":if ubuntu {"libssl3t64"} else {"libssl3"},
    "working_dir":context.build_dir.join("api"),"exec":exec,"redis_port":c.redis.port,
    "redis_password":secret.as_ref().map(|s|s.redis.as_str()).unwrap_or(""),"env_content":env_content,
    "nginx_location":location,"nginx_http":!tls_ready,"nginx_tls":tls_ready,"tls":c.tls.enabled,"no_tls":!c.tls.enabled,"domain":c.domain,"email":c.tls.email,
    "service_exists":Path::new(&format!("/etc/systemd/system/{id}.service")).exists()});
  publish(path, &serde_json::to_vec(&value)?, 0o600)
}
fn command(program: &str, args: &[&str]) -> Result<()> {
  ensure!(
    Command::new(program).args(args).status()?.success(),
    "Native command failed: {program}"
  );
  Ok(())
}
fn account(c: &Stack) -> Result<()> {
  // Refuse to borrow a human/system account merely because its name matches.
  let passwd = fs::read_to_string("/etc/passwd")?;
  if let Some(row) = passwd.lines().find(|l| l.split(':').next() == Some(&c.id)) {
    let fields: Vec<_> = row.split(':').collect();
    ensure!(
      fields.len() == 7
        && fields[4] == format!("doinstall-{}", c.id)
        && fields[5] == format!("/var/lib/{}", c.id)
        && fields[6] == "/usr/sbin/nologin",
      "Existing service account is unowned."
    );
  } else {
    for path in [
      format!("/etc/{}", c.id),
      format!("/var/lib/{}", c.id),
      format!("/var/log/{}", c.id),
    ] {
      ensure!(
        !Path::new(&path).exists(),
        "Refusing pre-existing state for an unowned account."
      );
    }
    ensure!(
      !fs::read_to_string("/etc/group")?
        .lines()
        .any(|l| l.split(':').next() == Some(&c.id)),
      "Existing service group is unowned."
    );
    command(
      "/usr/sbin/useradd",
      &[
        "--system",
        "--user-group",
        "--home-dir",
        &format!("/var/lib/{}", c.id),
        "--shell",
        "/usr/sbin/nologin",
        "--comment",
        &format!("doinstall-{}", c.id),
        &c.id,
      ],
    )?;
  }
  Ok(())
}
fn sql(database: &str, text: &str) -> Result<String> {
  let mut child = Command::new("/usr/sbin/runuser")
    .args([
      "-u",
      "postgres",
      "--",
      "/usr/bin/psql",
      "-X",
      "--no-password",
      "--set",
      "ON_ERROR_STOP=1",
      "--tuples-only",
      "--no-align",
      "--dbname",
      database,
    ])
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::null())
    .spawn()?;
  child.stdin.take().unwrap().write_all(text.as_bytes())?;
  let result = child.wait_with_output()?;
  ensure!(
    result.status.success(),
    "PostgreSQL operation failed; no SQL or credentials were logged."
  );
  Ok(String::from_utf8(result.stdout)?)
}
fn postgres(c: &Stack) -> Result<()> {
  let secret = read_credentials(c)?.context("Credentials are missing.")?;
  let marker = format!("doinstall:{}", c.id);
  let role = &c.database.role;
  let existing = sql(
    "postgres",
    &format!("SELECT COALESCE(shobj_description(oid,'pg_authid'),'') FROM pg_roles WHERE rolname='{role}';"),
  )?;
  let role_exists = sql("postgres", &format!("SELECT count(*) FROM pg_roles WHERE rolname='{role}';"))?.trim() == "1";
  ensure!(
    !role_exists || existing.trim() == marker,
    "Existing PostgreSQL role is unowned."
  );
  let databases: BTreeSet<_> = c.database.connections.values().collect();
  // Check all databases before creating any role/database.
  for db in &databases {
    let row = sql("postgres",&format!("SELECT pg_get_userbyid(datdba)||'|'||COALESCE(shobj_description(oid,'pg_database'),'') FROM pg_database WHERE datname='{db}';"))?;
    ensure!(
      row.trim().is_empty() || row.trim() == format!("{role}|{marker}"),
      "Existing PostgreSQL database is unowned."
    );
  }
  if !role_exists {
    sql(
      "postgres",
      &format!(
        "BEGIN; CREATE ROLE {role} LOGIN PASSWORD '{}'; COMMENT ON ROLE {role} IS '{marker}'; COMMIT;",
        secret.database
      ),
    )?;
  }
  for db in databases {
    let exists = sql("postgres", &format!("SELECT count(*) FROM pg_database WHERE datname='{db}';"))?.trim() == "1";
    if !exists {
      sql("postgres", &format!("CREATE DATABASE {db} OWNER {role};"))?;
      sql("postgres", &format!("COMMENT ON DATABASE {db} IS '{marker}';"))?;
    }
    // Detect password drift without resetting credentials or exposing secrets.
    let output = Command::new("/usr/bin/psql")
      .args([
        "-X",
        "--no-password",
        "-h",
        "127.0.0.1",
        "-U",
        role,
        "-d",
        db,
        "-c",
        "SELECT 1",
      ])
      .env("PGPASSWORD", &secret.database)
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status()?;
    ensure!(
      output.success(),
      "Application database login failed; refusing to reset its password."
    );
  }
  Ok(())
}
fn backup(c: &Stack, context: &ContextFile) -> Result<()> {
  if context.previous_build_dir.is_none() {
    return Ok(());
  }
  let stamp = context.build_dir.file_name().context("Missing build identity")?;
  let dir = PathBuf::from(format!("/var/backups/{}", c.id)).join(stamp);
  directory(&dir, 0o700)?;
  // Tenant databases may be created after the first installation. Back up every
  // database owned by the verified application role, not just its entry contexts.
  let names = sql("postgres", &format!("SELECT datname FROM pg_database WHERE datdba=(SELECT oid FROM pg_roles WHERE rolname='{}') AND NOT datistemplate ORDER BY datname;", c.database.role))?;
  for db in names.lines().filter(|name| !name.is_empty()) {
    ensure!(
      token(db, "_", 48) && !["postgres", "template0", "template1"].contains(&db),
      "Unsafe application database name."
    );
    let path = dir.join(format!("{db}.dump"));
    safe(&path)?;
    if !path.exists() {
      let mut options = OpenOptions::new();
      options.write(true).create_new(true);
      #[cfg(unix)]
      {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
      }
      let file = options.open(&path)?;
      let status = Command::new("/usr/sbin/runuser")
        .args([
          "-u",
          "postgres",
          "--",
          "/usr/bin/pg_dump",
          "--format=custom",
          "--dbname",
          db,
        ])
        .stdout(Stdio::from(file))
        .status()?;
      if !status.success() {
        fs::remove_file(&path)?;
        bail!("Database backup failed.");
      }
    }
    let verify = format!("di_verify_{}", &random()?[..20]);
    sql("postgres", &format!("CREATE DATABASE {verify};"))?;
    let restore = Command::new("/usr/sbin/runuser")
      .args([
        "-u",
        "postgres",
        "--",
        "/usr/bin/pg_restore",
        "--exit-on-error",
        "--no-owner",
        "--no-privileges",
        "--dbname",
        &verify,
      ])
      .stdin(Stdio::from(File::open(&path)?))
      .stdout(Stdio::null())
      .stderr(Stdio::null())
      .status();
    let dropped = sql("postgres", &format!("DROP DATABASE {verify};"));
    ensure!(
      restore?.success(),
      "Backup restore verification failed; migration was not started."
    );
    dropped?;
    println!("Restore-verified backup: {}", path.display());
  }
  Ok(())
}
fn migrate(c: &Stack, context: &ContextFile) -> Result<()> {
  if let Some(migration) = &c.migrations {
    let path = context.build_dir.join(&migration.executable);
    safe(&path)?;
    ensure!(path.is_file(), "Migration executable is missing.");
    let secret = read_credentials(c)?.context("Credentials are missing.")?;
    // This is application-owned code; its output is deliberately attached.
    ensure!(
      Command::new(path)
        .args(&migration.arguments)
        .current_dir(context.build_dir.join("api"))
        .envs(environment(c, context, &secret))
        .envs(&migration.environment)
        .status()?
        .success(),
      "Application migration failed."
    );
  }
  Ok(())
}
fn readiness(c: &Stack) -> Result<()> {
  let deadline = Instant::now() + Duration::from_secs(90);
  let address = SocketAddr::from(([127, 0, 0, 1], c.port));
  loop {
    let ready = (|| -> Result<bool> {
      let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))?;
      stream.set_read_timeout(Some(Duration::from_secs(2)))?;
      stream.set_write_timeout(Some(Duration::from_secs(2)))?;
      write!(
        stream,
        "GET {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n\r\n",
        c.health_path, c.domain
      )?;
      let mut bytes = [0; 256];
      let count = stream.read(&mut bytes)?;
      let text = std::str::from_utf8(&bytes[..count])?;
      Ok(text.split_whitespace().nth(1).and_then(|s| s.parse::<u16>().ok()) == Some(c.health_status))
    })()
    .unwrap_or(false);
    if ready {
      println!("API readiness passed.");
      return Ok(());
    }
    ensure!(Instant::now() < deadline, "API readiness failed; installation remains pending.");
    std::thread::sleep(Duration::from_secs(1));
  }
}
fn proxy_readiness(c: &Stack) -> Result<()> {
  let expected = if c.tls.enabled { 301 } else { c.health_status };
  let output = Command::new("/usr/bin/curl")
    .args([
      "--silent",
      "--show-error",
      "--max-time",
      "10",
      "--output",
      "/dev/null",
      "--write-out",
      "%{http_code}",
      "--header",
      &format!("Host: {}", c.domain),
      &format!("http://127.0.0.1{}", c.health_path),
    ])
    .output()?;
  ensure!(
    output.status.success() && String::from_utf8(output.stdout)?.trim() == expected.to_string(),
    "Nginx HTTP readiness failed."
  );
  if c.tls.enabled {
    ensure!(
      Command::new("/usr/bin/curl")
        .args([
          "--fail",
          "--silent",
          "--show-error",
          "--max-time",
          "10",
          "--resolve",
          &format!("{}:443:127.0.0.1", c.domain),
          &format!("https://{}{}", c.domain, c.health_path)
        ])
        .stdout(Stdio::null())
        .status()?
        .success(),
      "Nginx TLS readiness failed."
    );
  }
  println!("Nginx readiness passed.");
  Ok(())
}

#[cfg(test)]
mod tests {
  use super::*;
  fn fixture() -> Stack {
    serde_json::from_value(json!({"schema":1,"id":"example-api","domain":"example.test","port":5080,"executable":"api/Example.API","database":{"role":"example","connections":{"DATABASE_URL":"example"}},"redis":{"port":6380,"host-environment":"REDIS_HOST","port-environment":"REDIS_PORT","password-environment":"REDIS_PASSWORD"}})).unwrap()
  }
  #[test]
  fn contract_rejects_injection_collisions_and_system_databases() {
    let c = fixture();
    validate(&c).unwrap();
    for bad in ["x;DROP DATABASE postgres", "../postgres", "postgres"] {
      let mut x = c.clone();
      x.database.connections.insert("SECOND".into(), bad.into());
      assert!(validate(&x).is_err());
    }
    let mut x = c.clone();
    x.environment.insert("REDIS_PASSWORD".into(), "override".into());
    assert!(validate(&x).is_err());
    let mut x = c.clone();
    x.redis.port = x.port;
    assert!(validate(&x).is_err());
    let mut x = c.clone();
    x.executable = "api/../../bin/sh".into();
    assert!(validate(&x).is_err());
    let mut x = c.clone();
    x.domain = "example.test;\nroot /".into();
    assert!(validate(&x).is_err());
    let mut x = c;
    x.tls.enabled = true;
    assert!(validate(&x).is_err());
  }
  #[test]
  fn managed_files_are_unique_and_persistent() {
    assert!(env_key("ConnectionStrings__Application"));
    assert!(!env_key("BAD=KEY"));
    let files = managed_files(&fixture());
    assert_eq!(files.len(), 8);
    assert_eq!(files.iter().collect::<BTreeSet<_>>().len(), 8);
    assert!(files.iter().all(|s| s.starts_with("/etc/") && !s.contains("/app/")));
  }
  #[test]
  fn environment_separates_shared_stack_from_application_keys() {
    let c = fixture();
    let context = ContextFile {
      schema: 1,
      application_id: c.id.clone(),
      install_dir: "/opt/example-api".into(),
      build_dir: "/opt/example-api/app/one".into(),
      previous_build_dir: None,
    };
    let secret = Credentials {
      schema: 1,
      application: c.id.clone(),
      database: "d".repeat(64),
      redis: "e".repeat(64),
    };
    let env = environment(&c, &context, &secret);
    assert_eq!(env["REDIS_PORT"], "6380");
    assert!(env["DATABASE_URL"].contains("Database=example;"));
    assert_eq!(env["DOTNET_ROOT"], context.build_dir.join("runtime").display().to_string());
  }
  #[test]
  fn systemd_arguments_remain_single_tokens() {
    assert_eq!(systemd_argument("a \"b\" $c %d"), "\"a \\\"b\\\" $$c %%d\"");
  }
  #[test]
  fn unknown_fields_are_not_silently_ignored() {
    let mut value = serde_json::to_value(fixture()).unwrap();
    value["databsae"] = json!({});
    assert!(serde_json::from_value::<Stack>(value).is_err());
  }
}
