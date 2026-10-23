//! Best-effort, session-scoped terminal presentation. This module never launches
//! another terminal, changes a profile, or makes installation depend on the host.
#[cfg(windows)]
mod native;

use crate::config::TerminalOptions;
use std::{
  io::{IsTerminal, Write},
  path::Path,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Host {
  WindowsTerminal,
  Xterm,
  Iterm,
  AppleTerminal,
  Kitty,
  Vt,
  Unknown,
}

fn host(mut get: impl FnMut(&str) -> Option<String>) -> (Host, bool) {
  let remote = ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY", "TMUX", "STY"]
    .into_iter()
    .any(|key| get(key).is_some_and(|s| !s.is_empty()));
  let term = get("TERM").unwrap_or_default();
  let program = get("TERM_PROGRAM").unwrap_or_default();
  let has = |value: Option<String>| value.is_some_and(|s| !s.is_empty());
  // A remote or multiplexed shell may inherit hints from a different terminal.
  // Keep its broadly supported title requests, but never local window changes.
  let detected = if has(get("WT_SESSION")) {
    Host::WindowsTerminal
  } else if has(get("KITTY_WINDOW_ID")) {
    Host::Kitty
  } else if program == "iTerm.app" {
    Host::Iterm
  } else if program == "Apple_Terminal" {
    Host::AppleTerminal
  } else if has(get("XTERM_VERSION")) && term.starts_with("xterm") {
    Host::Xterm
  } else if term.starts_with("xterm")
    || term.starts_with("screen")
    || term.starts_with("tmux")
    || program == "vscode"
    || program == "WezTerm"
    || program == "ghostty"
  {
    Host::Vt
  } else {
    Host::Unknown
  };
  (detected, remote)
}

pub(crate) fn safe_title(text: &str) -> String {
  // Package metadata and explicit titles must never inject terminal commands.
  text.chars().filter(|c| !c.is_control()).take(256).collect()
}

fn grow(current: (u16, u16), preferred: (u16, u16)) -> (u16, u16) {
  (current.0.max(preferred.0), current.1.max(preferred.1))
}

fn restore_size(current: (u16, u16), applied: (u16, u16), before: (u16, u16)) -> Option<(u16, u16)> {
  // Do not undo a resize made by the person using the installer.
  (current == applied && before != applied).then_some(before)
}

#[derive(Default)]
pub(crate) struct Session {
  restore_title: bool,
  size: Option<((u16, u16), (u16, u16))>,
  #[cfg(windows)]
  native: Option<native::Session>,
}
impl Session {
  pub(crate) fn begin(name: &str, uninstall: bool, options: &TerminalOptions, icon: Option<&Path>) -> Self {
    let mut session = Self::default();
    if options.disabled || !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
      return session;
    }
    let default_title = format!("{name} {}", if uninstall { "Uninstall" } else { "Setup" });
    let title = safe_title(options.title.as_deref().unwrap_or(&default_title));
    let (host, remote) = host(|key| std::env::var(key).ok());
    #[cfg(windows)]
    if !remote {
      // Native title APIs also work through ConPTY. Icon/font/size APIs are
      // separately gated on a confirmed visible classic console window.
      session.native = native::Session::begin(&title, options, icon);
      if session.native.is_some() && host != Host::WindowsTerminal {
        return session;
      }
    }
    #[cfg(not(windows))]
    let _ = icon;
    if host == Host::Unknown {
      return session;
    }
    let mut output = std::io::stdout().lock();
    let native_title = {
      #[cfg(windows)]
      {
        session.native.is_some()
      }
      #[cfg(not(windows))]
      {
        false
      }
    };
    if !native_title {
      session.restore_title = !remote && matches!(host, Host::Xterm | Host::Iterm);
      if session.restore_title {
        let _ = write!(output, "\x1b[22;2t");
      }
      let _ = write!(output, "\x1b]2;{title}\x1b\\");
    }
    // These identified hosts document XTWINOPS resize support. Older versions
    // or host policy may ignore it; TERM alone never authorizes window changes.
    if !remote
      && matches!(host, Host::Xterm | Host::WindowsTerminal | Host::Iterm)
      && let Ok(before) = cursive::backends::crossterm::crossterm::terminal::size()
    {
      let applied = grow(before, options.size.unwrap_or((80, 24)));
      if applied != before {
        let _ = write!(output, "\x1b[8;{};{}t", applied.1, applied.0);
        session.size = Some((before, applied));
      }
    }
    let _ = output.flush();
    session
  }
}
impl Drop for Session {
  fn drop(&mut self) {
    let mut output = std::io::stdout().lock();
    if let Some((before, applied)) = self.size
      && let Ok(current) = cursive::backends::crossterm::crossterm::terminal::size()
      && let Some((columns, rows)) = restore_size(current, applied, before)
    {
      let _ = write!(output, "\x1b[8;{rows};{columns}t");
    }
    if self.restore_title {
      let _ = write!(output, "\x1b[23;2t");
    }
    let _ = output.flush();
  }
}

#[cfg(test)]
mod tests {
  use super::*;
  use crate::config::Options;
  use std::collections::BTreeMap;

  #[test]
  fn shell_name_and_generic_term_never_admit_window_operations() {
    let environment = BTreeMap::from([("TERM", "xterm-256color"), ("SHELL", "pwsh.exe")]);
    assert_eq!(host(|key| environment.get(key).map(|s| s.to_string())), (Host::Vt, false));
    assert_eq!(host(|_| None), (Host::Unknown, false));
    let remote = BTreeMap::from([
      ("XTERM_VERSION", "XTerm(411)"),
      ("TERM", "xterm"),
      ("SSH_CONNECTION", "remote"),
    ]);
    assert_eq!(host(|key| remote.get(key).map(|s| s.to_string())), (Host::Xterm, true));
  }

  #[test]
  fn terminal_preferences_stay_out_of_installer_values_and_reject_bad_numbers() {
    let parsed = Options::parse(
      [
        "--terminal-title",
        "Example Setup",
        "--terminal-size",
        "100x30",
        "--terminal-font-size",
        "20",
        "--terminal-icon",
        "missing icon.ico",
        "--no-terminal-customization",
      ]
      .map(str::to_owned),
    )
    .unwrap();
    assert!(parsed.values.is_empty());
    assert_eq!(parsed.terminal.size, Some((100, 30)));
    assert_eq!(parsed.terminal.font_size, Some(20));
    assert!(parsed.terminal.disabled);
    for args in [
      vec!["--terminal-size", "80x0"],
      vec!["--terminal-size", "80x24x1"],
      vec!["--terminal-font-size", "65536"],
      vec!["--terminal-size", "80x24", "--terminal-size", "90x30"],
      vec!["--terminal-font-size", "20", "--terminal-font-size", "22"],
    ] {
      assert!(Options::parse(args.into_iter().map(str::to_owned)).is_err());
    }
  }

  #[test]
  fn titles_cannot_inject_control_sequences_and_size_changes_never_shrink() {
    assert_eq!(safe_title("App\x1b]2;injection\x07\n\r\u{009b}name"), "App]2;injectionname");
    assert_eq!(safe_title(&"界".repeat(300)).chars().count(), 256);
    assert_eq!(grow((120, 40), (80, 24)), (120, 40));
    assert_eq!(grow((60, 20), (80, 24)), (80, 24));
    assert_eq!(restore_size((100, 30), (80, 24), (60, 20)), None);
    assert_eq!(restore_size((80, 24), (80, 24), (60, 20)), Some((60, 20)));
  }
}
