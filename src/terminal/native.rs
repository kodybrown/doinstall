//! Classic Windows console changes are scoped to the actual console HWND.
//! Never target the foreground window or a Windows Terminal parent window.
use super::{grow, restore_size};
use crate::config::TerminalOptions;
use std::path::Path;
use windows::{
  core::PCWSTR,
  Win32::{
    Foundation::{HANDLE, HWND, LPARAM, WPARAM},
    System::Console::*,
    UI::{Shell::ExtractIconExW, WindowsAndMessaging::*},
  },
};

fn wide(text: &str) -> Vec<u16> {
  text.encode_utf16().chain(Some(0)).collect()
}
fn title() -> Option<String> {
  let mut text = vec![0u16; 32768];
  let length = unsafe { GetConsoleTitleW(&mut text) } as usize;
  (length > 0).then(|| String::from_utf16_lossy(&text[..length]))
}
fn info(output: HANDLE) -> Option<CONSOLE_SCREEN_BUFFER_INFO> {
  let mut value = CONSOLE_SCREEN_BUFFER_INFO::default();
  unsafe {
    GetConsoleScreenBufferInfo(output, &mut value).ok()?;
  }
  Some(value)
}
fn size(value: &CONSOLE_SCREEN_BUFFER_INFO) -> (u16, u16) {
  (
    (value.srWindow.Right - value.srWindow.Left + 1) as u16,
    (value.srWindow.Bottom - value.srWindow.Top + 1) as u16,
  )
}
fn font(output: HANDLE) -> Option<CONSOLE_FONT_INFOEX> {
  let mut value = CONSOLE_FONT_INFOEX {
    cbSize: std::mem::size_of::<CONSOLE_FONT_INFOEX>() as u32,
    ..Default::default()
  };
  unsafe {
    GetCurrentConsoleFontEx(output, false, &mut value).ok()?;
  }
  Some(value)
}
fn same_font(a: &CONSOLE_FONT_INFOEX, b: &CONSOLE_FONT_INFOEX) -> bool {
  a.dwFontSize == b.dwFontSize
    && a.FaceName == b.FaceName
    && a.FontFamily == b.FontFamily
    && a.FontWeight == b.FontWeight
}
fn message(window: HWND, message: u32, selector: usize, value: isize) -> Option<usize> {
  let mut result = 0usize;
  // Presentation must not hang installation if another process stops responding.
  let status = unsafe {
    SendMessageTimeoutW(
      window,
      message,
      WPARAM(selector),
      LPARAM(value),
      SMTO_ABORTIFHUNG,
      100,
      Some(&mut result),
    )
  };
  (status.0 != 0).then_some(result)
}
fn resize(output: HANDLE, desired: (u16, u16)) -> Option<CONSOLE_SCREEN_BUFFER_INFO> {
  let before = info(output)?;
  let buffer = COORD {
    X: before.dwSize.X.max(desired.0 as i16),
    Y: before.dwSize.Y.max(desired.1 as i16),
  };
  if buffer != before.dwSize {
    unsafe {
      SetConsoleScreenBufferSize(output, buffer).ok()?;
    }
  }
  let window = SMALL_RECT {
    Left: 0,
    Top: 0,
    Right: desired.0 as i16 - 1,
    Bottom: desired.1 as i16 - 1,
  };
  if unsafe { SetConsoleWindowInfo(output, true, &window) }.is_err() {
    // Roll back only the attempted buffer expansion, without truncating new
    // console output. Failure is harmless; retain the current terminal state.
    if info(output)
      .is_some_and(|current| current.dwSize == buffer && current.dwCursorPosition == before.dwCursorPosition)
    {
      let _ = unsafe { SetConsoleScreenBufferSize(output, before.dwSize) };
    }
    return None;
  }
  info(output)
}

struct Icon {
  selector: usize,
  owned: HICON,
  previous: usize,
}

pub(super) struct Session {
  output: HANDLE,
  window: HWND,
  title: Option<(String, String)>,
  geometry: Option<(CONSOLE_SCREEN_BUFFER_INFO, CONSOLE_SCREEN_BUFFER_INFO)>,
  font: Option<(CONSOLE_FONT_INFOEX, CONSOLE_FONT_INFOEX)>,
  icons: Vec<Icon>,
}
impl Session {
  pub(super) fn begin(requested_title: &str, options: &TerminalOptions, fallback_icon: Option<&Path>) -> Option<Self> {
    let output = unsafe { GetStdHandle(STD_OUTPUT_HANDLE).ok()? };
    let mut mode = CONSOLE_MODE::default();
    unsafe {
      GetConsoleMode(output, &mut mode).ok()?;
    }
    let mut session = Self {
      output,
      window: unsafe { GetConsoleWindow() },
      title: None,
      geometry: None,
      font: None,
      icons: vec![],
    };
    // GetConsoleTitleW cannot distinguish an empty title from failure. In that
    // case leave it alone instead of making a change we cannot safely restore.
    if let Some(before) = title()
      && unsafe { SetConsoleTitleW(PCWSTR(wide(requested_title).as_ptr())) }.is_ok()
    {
      session.title = Some((before, requested_title.to_owned()));
    }
    let mut class = [0u16; 64];
    let length = unsafe { GetClassNameW(session.window, &mut class) } as usize;
    let visible_classic = unsafe { IsWindowVisible(session.window).as_bool() }
      && String::from_utf16_lossy(&class[..length]) == "ConsoleWindowClass";
    if !visible_classic {
      return Some(session);
    }
    let original = info(output);
    if let Some(height) = options.font_size
      && let Some(before) = font(output)
    {
      let mut requested = before;
      requested.dwFontSize = COORD {
        X: 0,
        Y: height as i16,
      };
      if unsafe { SetCurrentConsoleFontEx(output, false, &requested) }.is_ok()
        && let Some(applied) = font(output)
      {
        session.font = Some((before, applied));
      }
    }
    let mut processes = [0u32; 8];
    let dedicated = unsafe { GetConsoleProcessList(&mut processes) } == 1;
    // Default sizing only touches a console dedicated to this process. An
    // explicit preference opts a reused console into the same grow-only rule.
    if (dedicated || options.size.is_some())
      && let Some(before) = original
      && let Some(current) = info(output)
    {
      let preferred = grow(size(&current), options.size.unwrap_or((80, 24)));
      if preferred != size(&current) {
        if let Some(applied) = resize(output, preferred) {
          session.geometry = Some((before, applied));
        }
      } else if session.font.is_some() {
        session.geometry = Some((before, current));
      }
    }
    let own_executable = std::env::current_exe().ok();
    let icon_path = options.icon.as_deref().or(fallback_icon).or(own_executable.as_deref());
    if let Some(path) = icon_path {
      let path = wide(&path.to_string_lossy());
      let mut large = HICON::default();
      let mut small = HICON::default();
      unsafe {
        ExtractIconExW(PCWSTR(path.as_ptr()), 0, Some(&mut large), Some(&mut small), 1);
      }
      for (selector, owned) in [(ICON_BIG as usize, large), (ICON_SMALL as usize, small)] {
        if owned.is_invalid() {
          continue;
        }
        if let Some(previous) = message(session.window, WM_GETICON, selector, 0) {
          // Retain ownership even after a timed-out set: the window might have
          // accepted the icon. Drop checks it before releasing the resource.
          session.icons.push(Icon {
            selector,
            owned,
            previous,
          });
          let _ = message(session.window, WM_SETICON, selector, owned.0 as isize);
        } else {
          let _ = unsafe { DestroyIcon(owned) };
        }
      }
    }
    Some(session)
  }
}
impl Drop for Session {
  fn drop(&mut self) {
    let geometry_untouched = self.geometry.as_ref().is_some_and(|(_, applied)| {
      info(self.output).is_some_and(|current| current.srWindow == applied.srWindow && current.dwSize == applied.dwSize)
    });
    if let Some((before, applied)) = &self.font
      && font(self.output).is_some_and(|current| same_font(&current, applied))
    {
      let _ = unsafe { SetCurrentConsoleFontEx(self.output, false, before) };
    }
    if geometry_untouched
      && let Some((before, applied)) = &self.geometry
      && restore_size(size(applied), size(applied), size(before)).is_some()
    {
      // Use the original viewport position. Only shrink a buffer we enlarged,
      // and only while it still has exactly the dimensions we applied.
      let _ = unsafe { SetConsoleWindowInfo(self.output, true, &before.srWindow) };
      if info(self.output)
        .is_some_and(|current| current.dwSize == applied.dwSize && current.dwCursorPosition == before.dwCursorPosition)
      {
        let _ = unsafe { SetConsoleScreenBufferSize(self.output, before.dwSize) };
      }
    }
    for icon in &self.icons {
      if let Some(current) = message(self.window, WM_GETICON, icon.selector, 0) {
        if current == icon.owned.0 as usize {
          let _ = message(self.window, WM_SETICON, icon.selector, icon.previous as isize);
        }
        // Never release a resource while the window might still reference it.
        if message(self.window, WM_GETICON, icon.selector, 0).is_some_and(|current| current != icon.owned.0 as usize) {
          let _ = unsafe { DestroyIcon(icon.owned) };
        }
      }
    }
    if let Some((before, applied)) = &self.title
      && title().as_ref() == Some(applied)
    {
      let _ = unsafe { SetConsoleTitleW(PCWSTR(wide(before).as_ptr())) };
    }
  }
}
