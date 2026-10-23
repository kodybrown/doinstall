//! Drive the shipped interactive runtime through a real Unix PTY. These are
//! protocol/fallback checks, not claims about a graphical emulator's chrome.
#![cfg(unix)]
mod support;
use serde_json::json;
use std::{
  fs::{self, File},
  io::{Read, Write},
  os::{
    fd::{AsRawFd, FromRawFd},
    unix::process::CommandExt,
  },
  process::{Child, Command, Stdio},
  time::{Duration, Instant},
};

struct OwnedChild(Child);
impl Drop for OwnedChild {
  fn drop(&mut self) {
    let _ = self.0.kill();
    let _ = self.0.wait();
  }
}

#[test]
fn interactive_title_resize_restore_and_unsupported_host_fallback_use_the_runtime() {
  for scenario in [
    "vt",
    "xterm",
    "iterm",
    "windows-terminal",
    "remote",
    "unknown",
    "disabled",
  ] {
    let temp = support::temporary();
    let payload = temp.path().join("payload");
    fs::create_dir(&payload).unwrap();
    fs::copy(env!("CARGO_BIN_EXE_doinstall"), payload.join("example")).unwrap();
    let root = temp.path().join("application");
    let config = temp.path().join("installer.yaml");
    fs::write(
      &config,
      serde_json::to_vec(&json!({
        "schema":1, "application":{"id":"terminal-probe","name":"Example","version":"1.0"},
        "platform":if cfg!(target_os="macos") {"macos"} else {"linux"},
        "architecture":if cfg!(target_arch="aarch64") {"arm64"} else {"x64"},
        "payload":"payload", "runtime":{"commands":[{"name":"example","executable":"example"}],
        "values":{"install-dir":root,"add-to-path":false}}
      }))
      .unwrap(),
    )
    .unwrap();

    let (mut master_fd, mut slave_fd) = (-1, -1);
    let initial = libc::winsize {
      ws_row: 24,
      ws_col: 80,
      ws_xpixel: 0,
      ws_ypixel: 0,
    };
    // openpty creates fixture-owned descriptors. Files immediately assume their
    // ownership, and only the child session receives the slave as its terminal.
    assert_eq!(
      unsafe { libc::openpty(&mut master_fd, &mut slave_fd, std::ptr::null_mut(), std::ptr::null(), &initial) },
      0
    );
    let mut master = unsafe { File::from_raw_fd(master_fd) };
    let slave = unsafe { File::from_raw_fd(slave_fd) };
    assert_eq!(unsafe { libc::fcntl(master_fd, libc::F_SETFD, libc::FD_CLOEXEC) }, 0);
    let flags = unsafe { libc::fcntl(master_fd, libc::F_GETFL) };
    assert!(flags >= 0);
    assert_eq!(unsafe { libc::fcntl(master_fd, libc::F_SETFL, flags | libc::O_NONBLOCK) }, 0);
    let mut command = Command::new(env!("CARGO_BIN_EXE_doinstall"));
    command.args(["--config"]).arg(&config).args([
      "--interactive",
      "--terminal-title",
      "Example\x1b\x07 Setup",
      "--terminal-size",
      "100x30",
      "--terminal-font-size",
      "20",
      "--terminal-icon",
      "unavailable.ico",
    ]);
    for key in [
      "WT_SESSION",
      "KITTY_WINDOW_ID",
      "TERM_PROGRAM",
      "SSH_CONNECTION",
      "SSH_CLIENT",
      "SSH_TTY",
      "TMUX",
      "STY",
      "XTERM_VERSION",
    ] {
      command.env_remove(key);
    }
    command.env(
      "TERM",
      if scenario == "unknown" {
        "dumb"
      } else {
        "xterm-256color"
      },
    );
    if matches!(scenario, "xterm" | "remote") {
      command.env("XTERM_VERSION", "XTerm(411)");
    }
    if scenario == "iterm" {
      command.env("TERM_PROGRAM", "iTerm.app");
    }
    if scenario == "windows-terminal" {
      command.env("WT_SESSION", "fixture");
    }
    if scenario == "remote" {
      command.env("SSH_CONNECTION", "fixture");
    }
    if scenario == "disabled" {
      command.arg("--no-terminal-customization");
    }
    command
      .stdin(Stdio::from(slave.try_clone().unwrap()))
      .stdout(Stdio::from(slave.try_clone().unwrap()))
      .stderr(Stdio::from(slave.try_clone().unwrap()));
    unsafe {
      command.pre_exec(|| {
        if libc::setsid() < 0 || libc::ioctl(0, libc::TIOCSCTTY, 0) < 0 {
          return Err(std::io::Error::last_os_error());
        }
        Ok(())
      });
    }
    let mut child = OwnedChild(command.spawn().unwrap());
    drop(slave);
    let deadline = Instant::now() + Duration::from_secs(15);
    let mut output = Vec::new();
    let (mut escaped, mut confirmed, mut resized) = (false, false, false);
    let mut choices_seen = None;
    let mut confirmation_seen = None;
    let status = loop {
      let mut buffer = [0u8; 8192];
      loop {
        match master.read(&mut buffer) {
          Ok(0) => break,
          Ok(count) => output.extend_from_slice(&buffer[..count]),
          Err(error) if error.kind() == std::io::ErrorKind::WouldBlock || error.raw_os_error() == Some(libc::EIO) => {
            break
          }
          Err(error) => panic!("PTY read failed: {error}"),
        }
      }
      let text = String::from_utf8_lossy(&output);
      if matches!(scenario, "xterm" | "iterm" | "windows-terminal") && !resized && text.contains("\x1b[8;30;100t") {
        let applied = libc::winsize {
          ws_row: 30,
          ws_col: 100,
          ws_xpixel: 0,
          ws_ypixel: 0,
        };
        assert_eq!(unsafe { libc::ioctl(master.as_raw_fd(), libc::TIOCSWINSZ, &applied) }, 0);
        resized = true;
      }
      if text.contains("Install to:") && choices_seen.is_none() {
        choices_seen = Some(Instant::now());
      }
      if !escaped && choices_seen.is_some_and(|seen: Instant| seen.elapsed() > Duration::from_millis(200)) {
        master.write_all(b"\x1b").unwrap();
        escaped = true;
      }
      if text.contains("Cancel installation?") && confirmation_seen.is_none() {
        confirmation_seen = Some(Instant::now());
      }
      if !confirmed && confirmation_seen.is_some_and(|seen: Instant| seen.elapsed() > Duration::from_millis(100)) {
        master.write_all(b"\t\r").unwrap();
        confirmed = true;
      }
      if let Some(status) = child.0.try_wait().unwrap() {
        break status;
      }
      assert!(Instant::now() < deadline, "Interactive PTY timed out in {scenario}: {text:?}");
      std::thread::sleep(Duration::from_millis(20));
    };
    assert_eq!(status.code(), Some(130), "Cancellation failed in {scenario}");
    assert!(!root.exists(), "Choices/cancellation must not start installation");
    let text = String::from_utf8_lossy(&output);
    let customizes = !matches!(scenario, "unknown" | "disabled");
    assert_eq!(
      text.contains("\x1b]2;Example Setup\x1b\\"),
      customizes,
      "Title mismatch in {scenario}"
    );
    assert_eq!(
      text.contains("\x1b[8;30;100t"),
      matches!(scenario, "xterm" | "iterm" | "windows-terminal"),
      "Resize mismatch in {scenario}"
    );
    assert_eq!(
      text.contains("\x1b[22;2t"),
      matches!(scenario, "xterm" | "iterm"),
      "Title save mismatch in {scenario}"
    );
    assert_eq!(
      text.contains("\x1b[23;2t"),
      matches!(scenario, "xterm" | "iterm"),
      "Title restore mismatch in {scenario}"
    );
    if matches!(scenario, "xterm" | "iterm" | "windows-terminal") {
      assert!(text.contains("\x1b[8;24;80t"), "Original size was not restored");
    }
  }
}
