//! Capture Cursive's actual cells and colors, not a separate mock renderer.
use super::*;
use std::sync::Mutex;

#[derive(Clone)]
struct Cell {
  text: char,
  colors: ColorPair,
}
struct Screen {
  size: Vec2,
  position: Vec2,
  colors: ColorPair,
  cells: Vec<Cell>,
}
impl Screen {
  fn new(size: Vec2) -> Self {
    let colors = ColorPair {
      front: Color::Dark(cursive::theme::BaseColor::White),
      back: Color::Dark(cursive::theme::BaseColor::Black),
    };
    Self {
      size,
      position: Vec2::zero(),
      colors,
      cells: vec![Cell { text: ' ', colors }; size.x * size.y],
    }
  }
  fn find(&self, text: &str) -> Vec2 {
    for y in (0..self.size.y).rev() {
      let line: String = self.cells[y * self.size.x..(y + 1) * self.size.x]
        .iter()
        .map(|c| c.text)
        .collect();
      if let Some(byte) = line.find(text) {
        return Vec2::new(line[..byte].chars().count(), y);
      }
    }
    panic!("Text not rendered: {text}");
  }
  fn cell(&self, position: Vec2) -> &Cell {
    &self.cells[position.y * self.size.x + position.x]
  }
  fn snapshot(&self, name: &str) {
    // Optional test artifacts stay outside source and do not affect product behavior.
    if let Some(directory) = std::env::var_os("DOINSTALL_UI_SNAPSHOT_DIR") {
      let color = |color| match color {
        Color::Rgb(r, g, b) => format!("#{r:02x}{g:02x}{b:02x}"),
        _ => "#000000".to_owned(),
      };
      let value = json!({ "width": self.size.x, "height": self.size.y,
        "cells": self.cells.iter().map(|cell| json!([cell.text.to_string(), color(cell.colors.front), color(cell.colors.back)])).collect::<Vec<_>>() });
      std::fs::create_dir_all(&directory).unwrap();
      std::fs::write(
        std::path::Path::new(&directory).join(format!("{name}.json")),
        serde_json::to_vec(&value).unwrap(),
      )
      .unwrap();
    }
  }
}
struct Recording(Arc<Mutex<Screen>>);
impl Backend for Recording {
  fn poll_event(&mut self) -> Option<Event> {
    None
  }
  fn set_title(&mut self, _: String) {}
  fn refresh(&mut self) {}
  fn has_colors(&self) -> bool {
    true
  }
  fn screen_size(&self) -> Vec2 {
    self.0.lock().unwrap().size
  }
  fn move_to(&self, position: Vec2) {
    self.0.lock().unwrap().position = position;
  }
  fn print(&self, text: &str) {
    let mut screen = self.0.lock().unwrap();
    for ch in text.chars() {
      let position = screen.position;
      let colors = screen.colors;
      if position.x < screen.size.x && position.y < screen.size.y {
        let index = position.y * screen.size.x + position.x;
        screen.cells[index] = Cell { text: ch, colors };
      }
      screen.position.x += unicode_width::UnicodeWidthChar::width(ch).unwrap_or(0);
    }
  }
  fn clear(&self, background: Color) {
    let mut screen = self.0.lock().unwrap();
    let colors = ColorPair {
      front: screen.colors.front,
      back: background,
    };
    screen.cells.fill(Cell { text: ' ', colors });
  }
  fn set_color(&self, colors: ColorPair) -> ColorPair {
    let mut screen = self.0.lock().unwrap();
    std::mem::replace(&mut screen.colors, colors)
  }
  fn set_effect(&self, _: Effect) {}
  fn unset_effect(&self, _: Effect) {}
}

#[test]
fn colored_form_buttons_checkbox_focus_and_mouse_use_real_controls() {
  let (_temp, mut siv, root) = fixture();
  if let Action::Install { package, .. } = &mut siv.user_data::<State>().unwrap().action {
    package.application.description = "An application packaged by DoInstall.".into();
  }
  siv.pop_layer();
  choices(&mut siv);
  let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(100, 40))));
  let mut runner = siv.runner(Box::new(Recording(screen.clone())));
  runner.refresh();
  runner.focus_name("install-dir").unwrap();
  runner.refresh();
  let normal = {
    let screen = screen.lock().unwrap();
    let button = screen.find(" Install ");
    let normal = screen.cell(button).colors;
    assert_ne!(
      normal.back,
      presentation::SURFACE,
      "Unfocused button must have a filled background"
    );
    assert!(
      !screen.cells.iter().any(|c| "┤├┌┐└┘─".contains(c.text)),
      "Stock borders or title separators leaked into form"
    );
    let caption = screen.find("Install to:");
    let checkbox = screen.find("[ ] Place an additional command");
    assert_eq!(checkbox.x, caption.x);
    assert_eq!(checkbox.x, screen.find(&root.to_string_lossy()).x);
    assert_eq!(screen.cell(caption).colors.front, Color::Rgb(157, 174, 195));
    assert_eq!(
      screen.cell(screen.find("An application packaged")).colors.front,
      Color::Rgb(157, 174, 195)
    );
    assert_eq!(
      screen.cell(screen.find("Place an additional command")).colors.front,
      Color::Rgb(235, 241, 248)
    );
    for row in screen.cells.chunks(screen.size.x) {
      let text: String = row.iter().map(|cell| cell.text).collect();
      assert!(
        !["Installation", "Commands", "Shortcuts", "Options"].contains(&text.trim()),
        "Generated group headings must be absent"
      );
    }
    screen.snapshot("choices");
    normal
  };
  runner.call_on_name("installer-dialog", |form: &mut Form| {
    let _ = form
      .view
      .set_focus(cursive::views::DialogFocus::Button(if cfg!(windows) { 0 } else { 1 }));
  });
  runner.refresh();
  {
    let screen = screen.lock().unwrap();
    assert_ne!(screen.cell(screen.find(" Install ")).colors.back, normal.back);
  }
  runner.focus_name("additional-command").unwrap();
  runner.refresh();
  let label_position = {
    let screen = screen.lock().unwrap();
    let label = screen.find("Place an additional command");
    assert_eq!(screen.cell(label).colors.back, Color::Rgb(57, 90, 128));
    assert_eq!(screen.cell(label).colors.front, Color::Rgb(235, 241, 248));
    assert_eq!(screen.cell(Vec2::new(label.x - 4, label.y)).text, '[');
    assert_eq!(label.x - 4, screen.find("Install to:").x);
    screen.snapshot("checkbox-focused");
    label
  };
  runner.on_event(Event::Mouse {
    offset: Vec2::zero(),
    position: label_position,
    event: cursive::event::MouseEvent::Press(cursive::event::MouseButton::Left),
  });
  runner.on_event(Event::Mouse {
    offset: Vec2::zero(),
    position: label_position,
    event: cursive::event::MouseEvent::Release(cursive::event::MouseButton::Left),
  });
  assert_eq!(form_values(&mut runner)["additional-command"], true);
  runner.on_event(Event::Key(Key::Esc));
  runner.refresh();
  {
    let screen = screen.lock().unwrap();
    screen.find("Confirm");
    screen.find(" No ");
    assert!(!screen.cells.iter().any(|c| "┤├┌┐└┘─".contains(c.text)));
    screen.snapshot("cancel");
  }
  runner.on_event(Event::Key(Key::Enter));
  assert!(!runner.user_data::<State>().unwrap().confirming_cancel);
}

#[test]
fn wrapped_unicode_values_edit_without_newlines_and_keep_cursor_on_resize() {
  let (_temp, mut siv, _root) = fixture();
  let size = Arc::new(AtomicUsize::new(70));
  let mut runner = siv.runner(backend(size.clone()));
  runner.refresh();
  runner.focus_name("install-dir").unwrap();
  let text = "C:/very long path/日本語/é/".repeat(12);
  runner.call_on_name("install-dir", |view: &mut WrappedInput| {
    let _ = view.set_content(text.clone());
  });
  runner.refresh();
  runner.on_event(Event::Ctrl(Key::End));
  runner.on_event(Event::Char('界'));
  runner.refresh();
  for event in [
    Event::Key(Key::Enter),
    Event::Char('\n'),
    Event::Char('\r'),
    Event::Char('\t'),
  ] {
    runner.on_event(event);
  }
  let (value, cursor, rows) = runner
    .call_on_name("install-dir", |view: &mut WrappedInput| {
      (view.get_content().to_owned(), view.get_cursor(), view.size.y)
    })
    .unwrap();
  assert_eq!(value, format!("{text}界"));
  assert!(rows > 1);
  assert_eq!(cursor, value.len());
  size.store(160, Ordering::SeqCst);
  runner.on_event(Event::WindowResize);
  runner.refresh();
  assert_eq!(
    runner
      .call_on_name("install-dir", |view: &mut WrappedInput| (
        view.get_content().to_owned(),
        view.get_cursor()
      ))
      .unwrap(),
    (value, cursor)
  );
  runner.on_event(Event::Key(Key::Tab));
  runner.on_event(Event::Char(' '));
  assert_eq!(form_values(&mut runner)["additional-command"], true);
}

#[test]
fn warnings_errors_and_long_summary_text_share_the_theme_and_wrap() {
  let (_temp, mut siv, _root) = fixture();
  let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(76, 32))));
  let mut runner = siv.runner(Box::new(Recording(screen.clone())));
  runner.refresh();
  report(
    &mut runner,
    OperationEvent::Warning("Check the chosen command location.".into()),
  );
  report(
    &mut runner,
    OperationEvent::Message(format!("Application: C:/{}end-marker", "long-directory/".repeat(9))),
  );
  finished(&mut runner, Ok(()));
  runner.refresh();
  {
    let screen = screen.lock().unwrap();
    let warning = screen.find("Warning:");
    assert_eq!(screen.cell(warning).colors.front, presentation::WARNING);
    assert!(screen.find("end-marker").y > screen.find("Application:").y);
    screen.find(" Close ");
    screen.snapshot("summary-warning");
  }
  finished(&mut runner, Err(anyhow!("Cannot write the application directory.")));
  runner.refresh();
  {
    let screen = screen.lock().unwrap();
    assert_eq!(screen.cell(screen.find("Installer error:")).colors.front, presentation::ERROR);
    screen.snapshot("summary-error");
  }
}

#[test]
fn group_and_theme_contracts_are_strict_and_preserve_defaults() {
  let (_temp, mut siv, _root) = fixture();
  let package = siv.user_data::<State>().unwrap().action.package();
  let mut value = serde_json::to_value(package).unwrap();
  assert!(value["runtime"].get("theme").is_none());
  assert!(value["runtime"]["inputs"]["channel"].get("group-index").is_none());
  value["runtime"]["theme"] = "default".into();
  value["runtime"]["inputs"]["channel"]["group-index"] = 1.into();
  let package: Package = serde_json::from_value(value.clone()).unwrap();
  assert_eq!(package.runtime.inputs["channel"].group_index, Some(1));
  let yaml = serde_saphyr::to_string(&package).unwrap();
  assert_eq!(
    serde_saphyr::from_str::<Package>(&yaml).unwrap().runtime.inputs["channel"].group_index,
    Some(1)
  );
  value["runtime"]["theme"] = "theme.yaml".into();
  assert!(serde_json::from_value::<Package>(value.clone()).is_err());
  value["runtime"]["theme"] = "default".into();
  value["runtime"]["inputs"]["channel"]["group-index"] = (-1).into();
  assert!(serde_json::from_value::<Package>(value.clone()).is_err());
  value["runtime"]["inputs"]["channel"]["group-index"] = 1.into();
  value["runtime"]["padding"] = 4.into();
  assert!(serde_json::from_value::<Package>(value).is_err());
}

#[test]
fn explicit_groups_reorder_controls_stably_without_changing_values() {
  let (_temp, mut siv, _root) = fixture();
  let mut package = siv.user_data::<State>().unwrap().action.package().clone();
  package.runtime.inputs.get_mut("channel").unwrap().group_index = Some(0);
  for (name, group, prompt) in [
    ("early", 5, "Another option"),
    ("late", 1, "Grouped option"),
  ] {
    package.runtime.inputs.insert(
      name.into(),
      serde_json::from_value(json!({
        "type": "boolean", "group-index": group, "prompt": prompt, "default": false
      }))
      .unwrap(),
    );
  }
  let options = Options::default();
  let values = config::default_values(&package, &options).unwrap();
  siv.pop_layer();
  setup(
    &mut siv,
    Action::Install {
      config: PathBuf::from("unused"),
      package,
    },
    options,
    values,
  );
  let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(100, 48))));
  let mut runner = siv.runner(Box::new(Recording(screen.clone())));
  runner.refresh();
  let fields = &runner.user_data::<State>().unwrap().fields;
  assert_eq!(fields.first().unwrap(), "install-dir");
  assert_eq!(fields[1], "channel");
  assert!(
    fields.iter().position(|name| name == "late").unwrap() < fields.iter().position(|name| name == "early").unwrap()
  );
  runner.focus_name("late").unwrap();
  runner.on_event(Event::Char(' '));
  let values = form_values(&mut runner);
  assert_eq!(values["late"], true);
  assert_eq!(values["early"], false);
  assert_eq!(values["channel"], "stable");
  let screen = screen.lock().unwrap();
  assert!(screen.find("Channel").y < screen.find("Grouped option").y);
  assert!(screen.find("Grouped option").y < screen.find("Another option").y);
}

#[test]
fn tab_skips_disabled_fields_and_cli_locked_checkboxes() {
  let (_temp, mut siv, _root) = fixture();
  let package = siv.user_data::<State>().unwrap().action.package().clone();
  let options = Options {
    values: BTreeMap::from([("add-to-path".into(), false.into())]),
    ..Options::default()
  };
  let values = config::default_values(&package, &options).unwrap();
  siv.pop_layer();
  setup(
    &mut siv,
    Action::Install {
      config: PathBuf::from("unused"),
      package,
    },
    options,
    values,
  );
  let mut runner = siv.runner(backend(Arc::new(AtomicUsize::new(100))));
  runner.refresh();
  runner.focus_name("additional-command").unwrap();
  // Channel remains enabled after the two disabled command controls.
  runner.on_event(Event::Key(Key::Tab));
  runner.on_event(Event::Key(Key::Down));
  assert_eq!(form_values(&mut runner)["channel"], "preview");
  assert_eq!(form_values(&mut runner)["add-to-path"], false);
  runner.focus_name("additional-command").unwrap();
  runner.on_event(Event::Char(' '));
  assert!(runner.focus_name("bin-dir").is_ok());
}

#[cfg(windows)]
#[test]
fn existing_path_is_checked_locked_and_restores_the_users_choice() {
  for choice in [false, true] {
    let (_temp, mut siv, root) = fixture();
    let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(100, 48))));
    let mut runner = siv.runner(Box::new(Recording(screen.clone())));
    runner.refresh();
    runner.call_on_name("add-to-path", |view: &mut Checkbox| {
      let _ = view.set_checked(choice);
    });
    // Inject read-only membership results instead of editing the user's PATH.
    refresh_dependencies_with_path_check(&mut runner, |directory| {
      assert_eq!(directory, root);
      Ok(true)
    });
    refresh_dependencies_with_path_check(&mut runner, |_| Ok(true));
    runner.refresh();
    let state = runner
      .call_on_name("add-to-path", |view: &mut Checkbox| (view.is_checked(), view.is_enabled()))
      .unwrap();
    assert_eq!(state, (true, false));
    assert_eq!(
      form_values(&mut runner)["add-to-path"],
      choice,
      "Showing membership must not create consent for a PATH write"
    );
    {
      let screen = screen.lock().unwrap();
      screen.find("[X] Add this directory");
      screen.find("already on PATH");
      screen.find("checked and disabled");
      screen.snapshot("already-on-path");
    }
    // Changing the application directory rechecks membership and restores the
    // remembered choice; repeated refreshes above must not overwrite it.
    let changed_root = root.with_file_name("other-application");
    runner.call_on_name("install-dir", |view: &mut WrappedInput| {
      let _ = view.set_content(changed_root.to_string_lossy());
    });
    refresh_dependencies_with_path_check(&mut runner, |directory| {
      assert_eq!(directory, changed_root);
      Ok(false)
    });
    assert_eq!(
      runner
        .call_on_name("add-to-path", |view: &mut Checkbox| (view.is_checked(), view.is_enabled()))
        .unwrap(),
      (choice, true)
    );
    assert_eq!(form_values(&mut runner)["add-to-path"], choice);
    assert!(!root.exists());
  }
}

#[cfg(windows)]
#[test]
fn cli_path_lock_remains_explained_and_safe_when_membership_changes() {
  let (_temp, mut siv, root) = fixture();
  let package = siv.user_data::<State>().unwrap().action.package().clone();
  let options = Options {
    values: BTreeMap::from([("add-to-path".into(), false.into())]),
    ..Options::default()
  };
  let values = config::default_values(&package, &options).unwrap();
  siv.pop_layer();
  setup(
    &mut siv,
    Action::Install {
      config: PathBuf::from("unused"),
      package,
    },
    options,
    values,
  );
  let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(100, 48))));
  let mut runner = siv.runner(Box::new(Recording(screen.clone())));
  runner.refresh();
  screen.lock().unwrap().find("locked by the command-line setting");
  refresh_dependencies_with_path_check(&mut runner, |_| Ok(true));
  assert_eq!(
    runner
      .call_on_name("add-to-path", |view: &mut Checkbox| (view.is_checked(), view.is_enabled()))
      .unwrap(),
    (true, false)
  );
  assert_eq!(form_values(&mut runner)["add-to-path"], false);
  // With an additional command selected, its directory is the PATH candidate.
  runner.call_on_name("additional-command", |view: &mut Checkbox| {
    let _ = view.set_checked(true);
  });
  refresh_dependencies_with_path_check(&mut runner, |directory| {
    assert_eq!(directory, root.parent().unwrap());
    Ok(false)
  });
  runner.refresh();
  assert_eq!(
    runner
      .call_on_name("add-to-path", |view: &mut Checkbox| (view.is_checked(), view.is_enabled()))
      .unwrap(),
    (false, false)
  );
  assert_eq!(form_values(&mut runner)["add-to-path"], false);
  screen.lock().unwrap().find("locked by the command-line setting");
  assert!(!root.exists());
}

#[test]
fn half_block_frames_have_half_row_shadows_and_blank_popup_text_margins() {
  let (_temp, mut siv, _root) = fixture();
  let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(100, 44))));
  let mut runner = siv.runner(Box::new(Recording(screen.clone())));
  runner.refresh();
  {
    let screen = screen.lock().unwrap();
    let title = screen.find("Probe · 1.0");
    let left = title.x - 2;
    let cap = Vec2::new(left, title.y - 1);
    assert_eq!(screen.cell(cap).text, '▄');
    assert_eq!(screen.cell(cap).colors.back, Color::Rgb(15, 21, 30));
    assert_eq!(screen.cell(Vec2::new(left + 1, title.y + 1)).text, '▀');
    let width = screen.cells[cap.y * screen.size.x + left..]
      .iter()
      .take_while(|cell| cell.text == '▄')
      .count();
    assert_eq!(screen.cell(Vec2::new(left + width, title.y)).colors.back, Color::Rgb(5, 9, 15));
    let bottom_shadow = (title.y + 2..screen.size.y)
      .find(|y| {
        let cell = screen.cell(Vec2::new(left + 1, *y));
        cell.text == '▀' && cell.colors.front == Color::Rgb(5, 9, 15)
      })
      .unwrap();
    assert_eq!(
      screen.cell(Vec2::new(left + 1, bottom_shadow)).colors.back,
      Color::Rgb(15, 21, 30)
    );
    let hints = screen.find("Tab:move  Space:toggle Esc:cancel");
    assert_eq!(hints.y, screen.find(" Install ").y);
    assert_eq!(hints.y, screen.find(" Cancel ").y);
  }
  runner.on_event(Event::Key(Key::Esc));
  runner.refresh();
  let screen = screen.lock().unwrap();
  let title = screen.find("Confirm");
  let question = screen.find("Cancel installation?");
  let cap = Vec2::new(title.x - 2, title.y - 1);
  assert_eq!(screen.cell(cap).text, '▄');
  assert_eq!(screen.cell(cap).colors.back, presentation::SURFACE);
  let width = screen.cells[cap.y * screen.size.x + cap.x..]
    .iter()
    .take_while(|cell| cell.text == '▄')
    .count();
  for y in [question.y - 1, question.y + 1] {
    for x in cap.x + 1..cap.x + width - 1 {
      let cell = screen.cell(Vec2::new(x, y));
      assert_eq!(cell.text, ' ');
      assert_eq!(cell.colors.back, presentation::SURFACE);
    }
  }
  assert_eq!(screen.cell(Vec2::new(cap.x + width, title.y)).colors.back, Color::Rgb(5, 9, 15));
}

#[test]
fn compact_footer_hints_never_overlap_the_action_buttons() {
  let (_temp, mut siv, _root) = fixture();
  let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(50, 40))));
  let mut runner = siv.runner(Box::new(Recording(screen.clone())));
  runner.refresh();
  let screen = screen.lock().unwrap();
  let hint = screen.find("Tab:move Esc:cancel");
  let install = screen.find(" Install ");
  let cancel = screen.find(" Cancel ");
  assert_eq!(hint.y, install.y);
  assert_eq!(hint.y, cancel.y);
  assert!(hint.x + "Tab:move Esc:cancel".len() < install.x.min(cancel.x));
  assert_eq!(install.x < cancel.x, cfg!(windows));
  screen.snapshot("compact-footer");
}

#[test]
fn both_platform_confirmation_orders_keep_no_default_and_yes_clickable() {
  for windows in [true, false] {
    let (_temp, mut siv, root) = fixture();
    siv.user_data::<State>().unwrap().confirming_cancel = true;
    siv.screen_mut().add_layer(Transparent(Form::popup(cancellation_dialog(
      "Cancel installation?".into(),
      windows,
    ))));
    let screen = Arc::new(Mutex::new(Screen::new(Vec2::new(100, 44))));
    let mut runner = siv.runner(Box::new(Recording(screen.clone())));
    runner.refresh();
    {
      let screen = screen.lock().unwrap();
      let yes = screen.find(" Yes ");
      let no = screen.find(" No ");
      assert_eq!(yes.y, no.y);
      assert_eq!(yes.x < no.x, windows);
      assert_eq!(screen.cell(no).colors.back, Color::Rgb(119, 205, 236));
      screen.snapshot(if windows {
        "confirm-windows"
      } else {
        "confirm-unix"
      });
    }
    runner.on_event(Event::Key(Key::Enter));
    assert!(!runner.user_data::<State>().unwrap().confirming_cancel);
    assert!(runner.user_data::<State>().unwrap().outcome.is_none());
    runner.user_data::<State>().unwrap().confirming_cancel = true;
    runner.screen_mut().add_layer(Transparent(Form::popup(cancellation_dialog(
      "Cancel installation?".into(),
      windows,
    ))));
    runner.refresh();
    let yes = screen.lock().unwrap().find(" Yes ");
    for event in [
      cursive::event::MouseEvent::Press(cursive::event::MouseButton::Left),
      cursive::event::MouseEvent::Release(cursive::event::MouseButton::Left),
    ] {
      runner.on_event(Event::Mouse {
        offset: Vec2::zero(),
        position: yes,
        event,
      });
    }
    assert!(!runner.is_running());
    assert!(runner
      .user_data::<State>()
      .unwrap()
      .outcome
      .as_ref()
      .unwrap()
      .as_ref()
      .unwrap_err()
      .is::<console::Canceled>());
    assert!(!root.exists());
  }
}
