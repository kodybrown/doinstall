//! A small Cursive form over the existing installer, not a second installation engine.
mod presentation;
use crate::{
  config::{self, Options},
  console::{self, Operation, OperationEvent},
  engine,
  model::{Input, Package, ThemeName},
  user_path,
};
use anyhow::{anyhow, Result};
use cursive::views::stack_view::Transparent;
use cursive::{
  event::{Event, EventResult, Key},
  theme::ColorStyle,
  traits::*,
  utils::markup::StyledString,
  view::{View, ViewWrapper},
  views::{BoxedView, Checkbox, Dialog, DummyView, EnableableView, LinearLayout, SelectView, TextView},
  Cursive, Printer, Vec2,
};
use presentation::{FocusRow, Form, WrappedChoices, WrappedInput};
use serde_json::Value;
use std::{
  collections::BTreeMap,
  path::PathBuf,
  sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
  },
  thread::JoinHandle,
};

#[derive(Clone)]
enum Action {
  Install { config: PathBuf, package: Package },
  Uninstall { root: PathBuf, package: Package },
}
impl Action {
  fn package(&self) -> &Package {
    match self {
      Self::Install { package, .. } | Self::Uninstall { package, .. } => package,
    }
  }
  fn uninstall(&self) -> bool {
    matches!(self, Self::Uninstall { .. })
  }
}
#[derive(PartialEq, Debug)]
enum Stage {
  Choices,
  Working,
  Summary,
}
struct State {
  action: Action,
  options: Options,
  inputs: BTreeMap<String, Input>,
  values: BTreeMap<String, Value>,
  fields: Vec<String>,
  derived_bin: bool,
  // A checked PATH box can describe existing membership rather than consent
  // to change PATH. Keep the user's choice separately while it is locked.
  path_choice: Option<bool>,
  cancel: Arc<AtomicBool>,
  interrupt: Arc<AtomicBool>,
  confirming_cancel: bool,
  stage: Stage,
  messages: Vec<(String, bool)>,
  outcome: Option<Result<()>>,
  worker: Option<JoinHandle<Result<()>>>,
}

// Size the themed form to the console, retaining its controls and values behind
// the too-small notice. Presentation settings are internal, not package options.
struct SizedDialog {
  view: BoxedView,
  fits: bool,
}
impl SizedDialog {
  fn new(view: impl View) -> Self {
    Self {
      view: BoxedView::new(Box::new(view)),
      fits: true,
    }
  }
}
impl ViewWrapper for SizedDialog {
  cursive::wrap_impl!(self.view: BoxedView);
  fn wrap_required_size(&mut self, available: Vec2) -> Vec2 {
    self.fits = available.x >= 48 && available.y >= 12;
    if self.fits {
      // Fill the usable width so wrapped text fields grow on maximize. Keep
      // a small outer margin and use only the height the content needs; tall
      // forms may expand while short progress/summary dialogs remain compact.
      let usable = Vec2::new(available.x.saturating_sub(4).max(48), available.y.saturating_sub(2).max(12));
      let content = self.view.required_size(usable);
      Vec2::new(usable.x, content.y.min(usable.y))
    } else {
      available
    }
  }
  fn wrap_layout(&mut self, size: Vec2) {
    if self.fits {
      self.view.layout(size);
    }
  }
  fn wrap_draw(&self, printer: &Printer) {
    if self.fits {
      self.view.draw(printer);
    } else {
      // Transparent layers inherit the stack's Background style, whose text
      // can match its fill. A resize notice must choose its own readable colors.
      printer.with_color(presentation::notice_style(), |p| {
        p.print((0, 0), "Terminal too small.");
        p.print((0, 1), "Resize to at least 50 columns x 14 rows.");
        p.print((0, 2), "Esc: cancel / close");
      });
    }
  }
  fn wrap_on_event(&mut self, event: Event) -> EventResult {
    if self.fits {
      self.view.on_event(event)
    } else {
      EventResult::Ignored
    }
  }
}

pub fn install(config: PathBuf, package: Package, options: Options) -> Result<()> {
  config::validate(&package)?;
  let values = config::default_values(&package, &options)?;
  run(Action::Install { config, package }, options, values)
}
pub fn uninstall(root: PathBuf, package: Package, options: Options) -> Result<()> {
  run(Action::Uninstall { root, package }, options, BTreeMap::new())
}
fn run(action: Action, options: Options, values: BTreeMap<String, Value>) -> Result<()> {
  let mut siv = cursive::crossterm();
  setup(&mut siv, action, options, values);
  // Signals request the same confirmation as keyboard/button cancellation;
  // they must not reach the worker's cancellation flag before approval.
  let interrupt = siv.user_data::<State>().unwrap().interrupt.clone();
  let _session = console::TuiSession::begin(interrupt);
  // Polling also notices external Ctrl+C requests when no keyboard event arrives.
  siv.set_autorefresh(true);
  siv.set_on_post_event(cursive::event::EventTrigger::any(), handle_interrupt);
  siv.try_run().map_err(|error| {
    // Backend initialization can fail after raw mode was enabled but before
    // Cursive owns a backend whose Drop would restore it.
    use cursive::backends::crossterm::crossterm::{cursor, event::DisableMouseCapture, execute, terminal};
    let _ = terminal::disable_raw_mode();
    let _ = execute!(
      std::io::stdout(),
      cursor::Show,
      DisableMouseCapture,
      terminal::LeaveAlternateScreen
    );
    anyhow!("Cannot initialize terminal UI: {error}. Use --plain for line prompts.")
  })?;
  let mut state = siv.take_user_data::<State>().unwrap();
  if let Some(worker) = state.worker.take() {
    let completed = worker.join().map_err(|_| anyhow!("Installer worker panicked"))?;
    // If the backend exited before processing the summary callback, report the
    // actual worker outcome rather than calling a completed install canceled.
    if state.outcome.is_none() {
      state.outcome = Some(completed);
    }
  }
  // Cursive has restored the terminal before printing a durable scrollback result.
  for (message, warning) in &state.messages {
    if *warning {
      console::diagnostic(message);
    } else {
      console::message(message);
    }
  }
  state.outcome.unwrap_or_else(|| Err(console::Canceled.into()))
}
fn setup(siv: &mut Cursive, action: Action, options: Options, values: BTreeMap<String, Value>) {
  siv.set_theme(match action.package().runtime.theme {
    ThemeName::Default => presentation::theme(),
  });
  let mut inputs = config::standard_inputs();
  inputs.extend(action.package().runtime.inputs.clone());
  let derived_bin = !values.contains_key("bin-dir");
  siv.set_user_data(State {
    action,
    options,
    inputs,
    values,
    fields: vec![],
    derived_bin,
    path_choice: None,
    cancel: Arc::new(AtomicBool::new(false)),
    interrupt: Arc::new(AtomicBool::new(false)),
    confirming_cancel: false,
    stage: Stage::Choices,
    messages: vec![],
    outcome: None,
    worker: None,
  });
  siv.add_global_callback(Key::Esc, cancel_or_close);
  // Cursive registrations append callbacks, including its default quit hook.
  // Clear that hook before installing a priority handler so Ctrl+C cannot quit
  // the event loop behind an otherwise visible cancellation confirmation.
  siv.clear_global_callbacks(Event::CtrlChar('c'));
  siv.set_on_pre_event(Event::CtrlChar('c'), cancel_or_close);
  choices(siv);
}
fn choices(siv: &mut Cursive) {
  let state = siv.user_data::<State>().unwrap();
  let package = state.action.package();
  let title = format!("{} · {}", package.application.name, package.application.version);
  let mut content = LinearLayout::vertical();
  for text in [
    &package.application.copyright,
    &package.application.description,
  ] {
    if !text.is_empty() {
      content.add_child(TextView::new(presentation::description(text.clone())));
    }
  }
  content.add_child(DummyView.fixed_height(1));
  if let Action::Uninstall { root, .. } = &state.action {
    content.add_child(TextView::new(presentation::description(format!(
      "Uninstall from {}",
      root.display()
    ))));
    content.add_child(TextView::new(presentation::description(
      "Application settings are preserved unless selected below.",
    )));
    let checkbox = Checkbox::new()
      .with_enabled(!state.options.settings_explicit)
      .with_checked(state.options.remove_settings);
    content.add_child(
      FocusRow::new(checkbox, "Remove application settings")
        .with_name("remove-settings")
        .full_width(),
    );
  } else {
    let mut names = vec![
      "install-dir",
      "additional-command",
      "bin-dir",
      "add-to-path",
      "desktop-shortcuts",
      "start-menu-shortcuts",
      "local-shortcuts",
      "shortcut-name",
      "start-menu-nested",
      "desktop-dir",
      "start-menu-dir",
    ];
    names.extend(
      state
        .inputs
        .keys()
        .map(String::as_str)
        .filter(|name| !names_contains_builtin(name)),
    );
    // Permission controls appear only when this package declares that capability.
    let mut fields: Vec<String> = names
      .into_iter()
      .filter(|name| visible(package, name))
      .filter(|name| *name != "confirm-install" && state.inputs[*name].prompt.is_some())
      .map(str::to_owned)
      .collect();
    fields.sort_by_key(|name| group_index(name, &state.inputs[name]));
    state.fields = fields.clone();
    let mut previous_group = None;
    let mut path_status_added = false;
    for name in fields {
      let input = &state.inputs[&name];
      let group = group_index(&name, input);
      if previous_group != Some(group) {
        if previous_group == Some(1) {
          content.add_child(TextView::new("").with_name("path-status"));
          path_status_added = true;
        }
        if previous_group.is_some() {
          content.add_child(DummyView.fixed_height(1));
        }
        previous_group = Some(group);
      }
      // Use the concise built-in form caption without replacing prompts owned
      // by an application or changing the plain-mode question wording.
      let label = if name == "install-dir" && !package.runtime.inputs.contains_key(&name) {
        "Install to:".to_owned()
      } else {
        input.prompt.as_deref().unwrap_or(&name).to_owned()
      };
      let raw = state.values.get(&name).and_then(|v| config::text(v).ok()).unwrap_or_default();
      let raw = config::expand_input(input, &raw, &state.values).unwrap_or(raw);
      let explicit = state.options.values.contains_key(&name)
        || (name == "additional-command" && state.options.values.contains_key("bin-dir"));
      if input.kind == "boolean" {
        let checkbox = Checkbox::new()
          .with_checked(raw == "true")
          .on_change(|s, _| refresh_dependencies(s))
          .with_enabled(!explicit);
        content.add_child(FocusRow::new(checkbox, label).with_name(name.clone()).full_width());
      } else {
        content.add_child(TextView::new(presentation::description(label)));
        let view = if input.kind == "choice" {
          let mut choices = SelectView::<String>::new();
          choices.add_item("Choose…", String::new());
          for choice in &input.choices {
            choices.add_item(choice, choice.clone());
          }
          let selected = input.choices.iter().position(|choice| choice == &raw).map_or(0, |i| i + 1);
          BoxedView::new(Box::new(
            WrappedChoices::new(choices.selected(selected))
              .with_name(name.clone())
              .scrollable()
              .max_height(4),
          ))
        } else {
          BoxedView::new(Box::new(
            WrappedInput::new(name.clone(), raw).with_name(name.clone()).full_width(),
          ))
        };
        content.add_child(
          EnableableView::new(view)
            .with_enabled(!explicit)
            .with_name(format!("enabled-{name}")),
        );
        content.add_child(DummyView.fixed_height(1));
      }
    }
    if !path_status_added {
      content.add_child(TextView::new("").with_name("path-status"));
    }
  }
  let action_label = if state.action.uninstall() {
    "Uninstall"
  } else {
    "Install"
  };
  let mut dialog = Dialog::around(content.scrollable()).title(title);
  if cfg!(windows) {
    dialog.add_button(action_label, start);
    dialog.add_button("Cancel", cancel_or_close);
  } else {
    dialog.add_button("Cancel", cancel_or_close);
    dialog.add_button(action_label, start);
  }
  let dialog = Form::new(dialog).instructions().with_name("installer-dialog");
  siv.screen_mut().add_layer(Transparent(SizedDialog::new(dialog)));
  refresh_dependencies(siv);
}
fn group_index(name: &str, input: &Input) -> u32 {
  input.group_index.unwrap_or(match name {
    "install-dir" => 0,
    "additional-command" | "bin-dir" | "add-to-path" => 1,
    name if names_contains_builtin(name) => 2,
    _ => 3,
  })
}
fn names_contains_builtin(name: &str) -> bool {
  [
    "install-dir",
    "additional-command",
    "bin-dir",
    "add-to-path",
    "desktop-shortcuts",
    "start-menu-shortcuts",
    "local-shortcuts",
    "shortcut-name",
    "start-menu-nested",
    "desktop-dir",
    "start-menu-dir",
  ]
  .contains(&name)
}
fn visible(package: &Package, name: &str) -> bool {
  match name {
    "add-to-path" => cfg!(windows),
    "desktop-shortcuts" | "desktop-dir" => package.runtime.shortcuts.iter().any(|s| s.desktop),
    "start-menu-shortcuts" | "start-menu-dir" => {
      !cfg!(target_os = "macos") && package.runtime.shortcuts.iter().any(|s| s.start_menu)
    }
    "local-shortcuts" => package.runtime.shortcuts.iter().any(|s| s.local),
    "start-menu-nested" => cfg!(windows) && package.runtime.shortcuts.iter().any(|s| s.start_menu),
    "shortcut-name" => package.runtime.shortcuts.iter().any(|s| s.name.contains("${shortcut-name}")),
    _ => true,
  }
}
fn form_values(siv: &mut Cursive) -> BTreeMap<String, Value> {
  let state = siv.user_data::<State>().unwrap();
  let mut values = state.values.clone();
  let fields = state.fields.clone();
  let inputs = state.inputs.clone();
  let path_choice = state.path_choice;
  for name in fields {
    let value = match inputs[&name].kind.as_str() {
      "boolean" if name == "add-to-path" && path_choice.is_some() => path_choice.map(Value::Bool),
      "boolean" => siv.call_on_name(&name, |view: &mut FocusRow| Value::Bool(view.is_checked())),
      "choice" => siv.call_on_name(&name, |view: &mut WrappedChoices| {
        Value::String(view.selection().map(|v| (*v).clone()).unwrap_or_default())
      }),
      _ => siv.call_on_name(&name, |view: &mut WrappedInput| Value::String(view.get_content().to_string())),
    };
    if let Some(value) = value {
      values.insert(name, value);
    }
  }
  values
}
fn refresh_dependencies(siv: &mut Cursive) {
  refresh_dependencies_with_path_check(siv, user_path::contains);
}
fn refresh_dependencies_with_path_check(siv: &mut Cursive, check_path: impl FnOnce(&std::path::Path) -> Result<bool>) {
  let state = siv.user_data::<State>().unwrap();
  if state.action.uninstall() || state.stage != Stage::Choices {
    return;
  }
  let fields = state.fields.clone();
  let derived = state.derived_bin;
  let options = state.options.clone();
  let mut values = form_values(siv);
  if derived
    && let Ok(root) = config::path(&values, "install-dir")
    && let Some(parent) = root.parent()
  {
    let suggested = parent.to_string_lossy().to_string();
    // Do not execute the edit callback for a programmatically derived default.
    siv.call_on_name("bin-dir", |view: &mut WrappedInput| {
      let _ = view.set_content(suggested.clone());
    });
    values.insert("bin-dir".into(), suggested.into());
  }
  let directory = config::command_directory(&values);
  let path_presence = match &directory {
    Ok(directory) if cfg!(windows) => Some(check_path(directory)),
    _ => None,
  };
  let already_on_path = matches!(&path_presence, Some(Ok(true)));
  if already_on_path {
    siv.user_data::<State>().unwrap().path_choice = Some(config::enabled(&values, "add-to-path"));
    siv.call_on_name("add-to-path", |view: &mut FocusRow| {
      // Ignore the callback: this is a presentation update, not a user edit.
      let _ = view.set_checked(true);
    });
  } else if let Some(choice) = siv.user_data::<State>().unwrap().path_choice.take() {
    siv.call_on_name("add-to-path", |view: &mut FocusRow| {
      let _ = view.set_checked(choice);
    });
  }
  for name in fields {
    let allowed = match name.as_str() {
      "add-to-path" => !already_on_path,
      "bin-dir" => config::enabled(&values, "additional-command"),
      "desktop-dir" => config::enabled(&values, "desktop-shortcuts"),
      "start-menu-dir" | "start-menu-nested" => config::enabled(&values, "start-menu-shortcuts"),
      "shortcut-name" => [
        "desktop-shortcuts",
        "start-menu-shortcuts",
        "local-shortcuts",
      ]
      .iter()
      .any(|key| config::enabled(&values, key)),
      _ => true,
    } && !options.values.contains_key(&name)
      && !(name == "additional-command" && options.values.contains_key("bin-dir"));
    let wrapper = format!("enabled-{name}");
    // Nonboolean fields use EnableableView for disabled presentation, but that
    // wrapper does not reject focus. Disable the actual controls too, so Tab
    // skips locked options; checkbox rows already use their control's policy.
    let kind = siv.user_data::<State>().unwrap().inputs[&name].kind.clone();
    match kind.as_str() {
      "boolean" => {
        siv.call_on_name(&name, |view: &mut FocusRow| view.set_enabled(allowed));
      }
      "choice" => {
        siv.call_on_name(&name, |view: &mut WrappedChoices| view.set_enabled(allowed));
      }
      _ => {
        siv.call_on_name(&name, |view: &mut WrappedInput| view.set_enabled(allowed));
      }
    }
    siv.call_on_name(&wrapper, |view: &mut EnableableView<BoxedView>| view.set_enabled(allowed));
  }
  let mut status = match path_presence {
      Some(Ok(true)) => "Selected command directory is already on PATH.\nThis option is checked and disabled because no PATH change is needed.".to_owned(),
      Some(Ok(false)) if config::enabled(&values, "add-to-path") => {
        "Selected command directory will be added to user PATH.".into()
      }
      Some(Ok(false)) => "Selected command directory will not be added to PATH.".into(),
      Some(Err(error)) => format!("Cannot check PATH: {error:#}"),
      None if directory.is_ok() => "Add the selected command directory to your shell PATH manually.".into(),
      None => "Enter an absolute installation and command directory.".into(),
  };
  if cfg!(windows) && !already_on_path && options.values.contains_key("add-to-path") {
    status.push_str("\nThis option is locked by the command-line setting.");
  }
  siv.call_on_name("path-status", |view: &mut TextView| {
    view.set_content(presentation::description(status))
  });
}
fn start(siv: &mut Cursive) {
  let state = siv.user_data::<State>().unwrap();
  if state.stage != Stage::Choices {
    return;
  }
  let action = state.action.clone();
  let mut options = state.options.clone();
  options.interactive = Some(false);
  if action.uninstall() {
    options.remove_settings = siv
      .call_on_name("remove-settings", |view: &mut FocusRow| view.is_checked())
      .unwrap_or(false);
    options.settings_explicit = true;
  } else {
    options.values = form_values(siv);
    if let Err(error) =
      config::resolve(action.package(), &options).and_then(|values| config::path(&values, "install-dir").map(|_| ()))
    {
      siv.screen_mut().add_layer(Transparent(SizedDialog::new(Form::popup(
        Dialog::around(
          TextView::new(StyledString::styled(
            format!("Check installation settings:\n{error:#}"),
            ColorStyle::new(presentation::ERROR, presentation::SURFACE),
          ))
          .scrollable(),
        )
        .title("Check installation settings")
        .button("OK", |s| {
          s.pop_layer();
        }),
      ))));
      return;
    }
  }
  let state = siv.user_data::<State>().unwrap();
  state.stage = Stage::Working;
  let cancel = state.cancel.clone();
  let title = action.package().application.name.clone();
  siv.pop_layer();
  siv.screen_mut().add_layer(Transparent(SizedDialog::new(Form::new(
    Dialog::around(
      LinearLayout::vertical()
        .child(TextView::new("Preparing…").with_name("progress"))
        .child(TextView::new("").with_name("working-warning"))
        .child(
          TextView::new(
            "Cancellation stops before destination changes begin.\nOnce changes begin, the operation finishes safely.",
          )
          .scrollable()
          .max_height(4),
        )
        .scrollable(),
    )
    .title(title)
    .button("Cancel", cancel_or_close),
  ))));
  let sink = siv.cb_sink().clone();
  let events = sink.clone();
  let operation = Operation {
    cancel: cancel.clone(),
    report: Arc::new(move |event| {
      let _ = events.send(Box::new(move |s| report(s, event)));
    }),
  };
  let worker = std::thread::spawn(move || {
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
      console::with_operation(operation, || match action {
        Action::Install { config, package } => engine::install(&config, package, &options),
        Action::Uninstall { root, .. } => engine::uninstall(&root, &options),
      })
    }))
    .unwrap_or_else(|_| Err(anyhow!("Installer worker panicked; inspect the installation before retrying.")));
    let summary = match &outcome {
      Ok(()) => Ok(()),
      Err(error) if error.is::<console::Canceled>() => Err(console::Canceled.into()),
      Err(error) => Err(anyhow!("{error:#}")),
    };
    let _ = sink.send(Box::new(move |s| finished(s, summary)));
    outcome
  });
  siv.user_data::<State>().unwrap().worker = Some(worker);
}
fn report(siv: &mut Cursive, event: OperationEvent) {
  match event {
    OperationEvent::Phase(label) => {
      siv.call_on_name("progress", |view: &mut TextView| view.set_content(label));
    }
    OperationEvent::Message(message) => {
      siv.user_data::<State>().unwrap().messages.push((message, false));
    }
    OperationEvent::Warning(message) => {
      siv.call_on_name("working-warning", |view: &mut TextView| {
        view.set_content(StyledString::styled(
          format!("Warning: {message}"),
          ColorStyle::new(presentation::WARNING, presentation::SURFACE),
        ));
      });
      siv.user_data::<State>().unwrap().messages.push((message, true));
    }
  }
}
fn finished(siv: &mut Cursive, outcome: Result<()>) {
  let state = siv.user_data::<State>().unwrap();
  state.stage = Stage::Summary;
  let result = match &outcome {
    Ok(()) => if state.cancel.load(Ordering::SeqCst) { "Completed. Cancellation arrived after destination changes began.".to_string() } else { "Completed.".to_string() },
    Err(error) if error.is::<console::Canceled>() => "Canceled before destination changes began.".into(),
    Err(error) => format!("Installer error:\n{error:#}\n\nIf activation or removal was interrupted, retry with the same package and locations."),
  };
  let title = state.action.package().application.name.clone();
  let result_color = match &outcome {
    Ok(()) => presentation::SUCCESS,
    Err(error) if error.is::<console::Canceled>() => presentation::WARNING,
    Err(_) => presentation::ERROR,
  };
  let mut summary = StyledString::styled(result, ColorStyle::new(result_color, presentation::SURFACE));
  for (message, warning) in &state.messages {
    summary.append_plain("\n\n");
    if *warning {
      summary.append_styled(
        format!("Warning: {message}"),
        ColorStyle::new(presentation::WARNING, presentation::SURFACE),
      );
    } else {
      summary.append_plain(message);
    }
  }
  state.outcome = Some(outcome);
  // The worker can finish while confirmation is open. Remove the modal before
  // replacing progress, so a stale Yes cannot act on the completed operation.
  let confirming = state.confirming_cancel;
  state.confirming_cancel = false;
  if confirming {
    siv.pop_layer();
  }
  siv.pop_layer();
  siv.screen_mut().add_layer(Transparent(SizedDialog::new(Form::new(
    Dialog::around(TextView::new(summary).scrollable())
      .title(format!("{title} — Summary"))
      .button("Close", |s| s.quit()),
  ))));
}
fn handle_interrupt(siv: &mut Cursive) {
  if siv
    .user_data::<State>()
    .is_some_and(|state| state.interrupt.swap(false, Ordering::SeqCst))
  {
    cancel_or_close(siv);
  }
}
fn cancel_or_close(siv: &mut Cursive) {
  let state = siv.user_data::<State>().unwrap();
  if state.stage == Stage::Summary {
    siv.quit();
    return;
  }
  if state.confirming_cancel {
    // Esc/Ctrl+C dismisses the confirmation as No, never as implicit consent.
    dismiss_cancel(siv);
    return;
  }
  if state.cancel.load(Ordering::SeqCst) {
    return;
  }
  if state.action.package().runtime.confirm_cancel {
    state.confirming_cancel = true;
    let action = if state.action.uninstall() {
      "uninstallation"
    } else {
      "installation"
    };
    let question = if state.stage == Stage::Working {
      format!(
        "Request cancellation of {action}?\nAn operation that has begun changing destinations may still finish safely."
      )
    } else {
      format!("Cancel {action}?")
    };
    // Platform order changes placement, never consent: No stays focused so
    // Enter preserves the operation on every platform.
    // Keep this modal compact and usable below the form's minimum size, so
    // the resize notice does not prevent the user from confirming cancellation.
    siv.screen_mut().add_layer(Transparent(
      Form::popup(cancellation_dialog(question, cfg!(windows))).with_name("cancel-confirmation"),
    ));
  } else {
    perform_cancel(siv);
  }
}
fn cancellation_dialog(question: String, windows: bool) -> Dialog {
  let mut dialog = Dialog::text(StyledString::styled(
    question,
    ColorStyle::new(presentation::WARNING, presentation::SURFACE),
  ))
  .title("Confirm");
  if windows {
    dialog.add_button("Yes", confirm_cancel);
    dialog.add_button("No", dismiss_cancel);
  } else {
    dialog.add_button("No", dismiss_cancel);
    dialog.add_button("Yes", confirm_cancel);
  }
  let _ = dialog.set_focus(cursive::views::DialogFocus::Button(if windows { 1 } else { 0 }));
  dialog
}
fn dismiss_cancel(siv: &mut Cursive) {
  if siv.user_data::<State>().unwrap().confirming_cancel {
    siv.user_data::<State>().unwrap().confirming_cancel = false;
    siv.pop_layer();
  }
}
fn confirm_cancel(siv: &mut Cursive) {
  dismiss_cancel(siv);
  perform_cancel(siv);
}
fn perform_cancel(siv: &mut Cursive) {
  let state = siv.user_data::<State>().unwrap();
  match state.stage {
    Stage::Choices => {
      state.outcome = Some(Err(console::Canceled.into()));
      siv.quit();
    }
    Stage::Working => {
      state.cancel.store(true, Ordering::SeqCst);
      siv.call_on_name("progress", |view: &mut TextView| {
        view.set_content("Cancellation requested; waiting for a safe stopping point…")
      });
    }
    Stage::Summary => siv.quit(),
  }
}

#[cfg(test)]
mod tests {
  mod rendering;
  use super::*;
  use cursive::{
    backend::{Backend, Dummy},
    theme::{Color, ColorPair, Effect},
  };
  use serde_json::json;
  use std::sync::atomic::AtomicUsize;

  // Exercise Cursive's real event dispatch/layout without an active terminal.
  struct Headless {
    size: Arc<AtomicUsize>,
    dummy: Dummy,
    output: Arc<std::sync::Mutex<String>>,
  }
  impl Backend for Headless {
    fn poll_event(&mut self) -> Option<Event> {
      None
    }
    fn set_title(&mut self, _: String) {}
    fn refresh(&mut self) {}
    fn has_colors(&self) -> bool {
      false
    }
    fn screen_size(&self) -> Vec2 {
      let width = self.size.load(Ordering::SeqCst);
      Vec2::new(width, if width < 48 { 8 } else { 32 })
    }
    fn move_to(&self, _: Vec2) {}
    fn print(&self, text: &str) {
      self.output.lock().unwrap().push_str(text);
    }
    fn clear(&self, _: Color) {}
    fn set_color(&self, pair: ColorPair) -> ColorPair {
      self.dummy.set_color(pair)
    }
    fn set_effect(&self, _: Effect) {}
    fn unset_effect(&self, _: Effect) {}
  }
  fn backend(size: Arc<AtomicUsize>) -> Box<dyn Backend> {
    Box::new(Headless {
      size,
      dummy: Dummy,
      output: Arc::new(std::sync::Mutex::new(String::new())),
    })
  }
  fn fixture() -> (tempfile::TempDir, Cursive, PathBuf) {
    // Hosted Windows can use an 8.3 TEMP path and macOS uses /var -> /private/var.
    // Create the fixture under the same resolved, native spelling the runtime
    // uses, so UI expectations and direct engine calls share one owned root.
    let temporary = std::env::temp_dir();
    let parent = crate::files::absolute(&temporary, &temporary).unwrap();
    let temp = tempfile::tempdir_in(parent).unwrap();
    let root = temp.path().join("application");
    let payload = temp.path().join("payload");
    std::fs::create_dir(&payload).unwrap();
    let executable = if cfg!(windows) { "probe.exe" } else { "probe" };
    std::fs::write(payload.join(executable), "test application").unwrap();
    #[cfg(unix)]
    {
      use std::os::unix::fs::PermissionsExt;
      std::fs::set_permissions(payload.join(executable), std::fs::Permissions::from_mode(0o755)).unwrap();
    }
    let package: Package = serde_json::from_value(json!({
      "schema":1, "application":{"id":"probe","name":"Probe","version":"1.0"},
      "platform": config::platform(), "architecture":config::architecture(), "payload":payload,
      "runtime":{"commands":[{"name":"probe","executable":executable}],
        "inputs":{"channel":{"type":"choice","required":true,"choices":["stable","preview"],"default":"stable","prompt":"Channel"}},
        "values":{"install-dir":root,"add-to-path":false}}
    })).unwrap();
    let mut siv = Cursive::new();
    let options = Options::default();
    let values = config::default_values(&package, &options).unwrap();
    setup(
      &mut siv,
      Action::Install {
        config: temp.path().join("source.yaml"),
        package,
      },
      options,
      values,
    );
    (temp, siv, root)
  }

  #[test]
  fn literal_application_templates_stay_literal_in_the_dialog() {
    let (_temp, mut existing, _root) = fixture();
    let mut action = existing.user_data::<State>().unwrap().action.clone();
    let Action::Install { package, .. } = &mut action else {
      unreachable!()
    };
    package.runtime.inputs.insert(
      "filename".into(),
      serde_json::from_value(json!({
        "type":"string","expand":false,"default":"${HOST}-${date:yyyyMMdd}.txt"
      }))
      .unwrap(),
    );
    let values = config::default_values(package, &Options::default()).unwrap();
    let mut siv = Cursive::new();
    setup(&mut siv, action, Options::default(), values);
    let mut runner = siv.runner(backend(Arc::new(AtomicUsize::new(100))));
    runner.refresh();
    assert_eq!(form_values(&mut runner)["filename"], "${HOST}-${date:yyyyMMdd}.txt");
  }
  #[test]
  fn tab_checkbox_dependencies_and_reverse_tab_use_standard_controls() {
    let (_temp, mut siv, _root) = fixture();
    let mut runner = siv.runner(backend(Arc::new(AtomicUsize::new(100))));
    runner.refresh();
    runner.focus_name("install-dir").unwrap();
    runner.on_event(Event::Key(Key::Tab));
    runner.on_event(Event::Char(' '));
    assert_eq!(form_values(&mut runner)["additional-command"], true);
    assert!(runner
      .call_on_name("enabled-bin-dir", |v: &mut EnableableView<BoxedView>| v.is_enabled())
      .unwrap());
    runner.on_event(Event::Key(Key::Tab));
    runner.on_event(Event::Shift(Key::Tab));
    runner.on_event(Event::Char(' '));
    assert_eq!(form_values(&mut runner)["additional-command"], false);
    assert!(!runner
      .call_on_name("enabled-bin-dir", |v: &mut EnableableView<BoxedView>| v.is_enabled())
      .unwrap());
  }
  #[test]
  fn console_growth_expands_rendered_text_fields_and_shrinking_preserves_edits() {
    let (_temp, mut siv, _root) = fixture();
    let size = Arc::new(AtomicUsize::new(90));
    let output = Arc::new(std::sync::Mutex::new(String::new()));
    let mut runner = siv.runner(Box::new(Headless {
      size: size.clone(),
      dummy: Dummy,
      output: output.clone(),
    }));
    runner.refresh();
    runner.focus_name("install-dir").unwrap();
    runner.on_event(Event::Key(Key::Left));
    let cursor = runner
      .call_on_name("install-dir", |view: &mut WrappedInput| view.get_cursor())
      .unwrap();
    let values = form_values(&mut runner);
    let narrow = runner.call_on_name("install-dir", |view: &mut WrappedInput| view.size).unwrap();
    output.lock().unwrap().clear();
    size.store(160, Ordering::SeqCst);
    runner.on_event(Event::WindowResize);
    runner.refresh();
    let wide = runner.call_on_name("install-dir", |view: &mut WrappedInput| view.size).unwrap();
    // Inspect the editor's allocated area rather than frame width: maximizing
    // must provide usable columns and shrinking must soft-wrap the same value.
    assert!(
      wide.x > narrow.x + 60,
      "Fields did not expand: narrow={narrow:?}, wide={wide:?}"
    );
    assert!(wide.y <= narrow.y);
    assert_eq!(form_values(&mut runner), values);
    assert_eq!(
      runner
        .call_on_name("install-dir", |view: &mut WrappedInput| view.get_cursor())
        .unwrap(),
      cursor
    );
    output.lock().unwrap().clear();
    size.store(90, Ordering::SeqCst);
    runner.on_event(Event::WindowResize);
    runner.refresh();
    assert_eq!(form_values(&mut runner), values);
    assert_eq!(
      runner.call_on_name("install-dir", |view: &mut WrappedInput| view.size).unwrap(),
      narrow
    );
  }
  #[test]
  fn location_changes_derive_bin_until_user_edits_it_and_resize_preserves_values() {
    let (temp, mut siv, _) = fixture();
    let size = Arc::new(AtomicUsize::new(100));
    let mut runner = siv.runner(backend(size.clone()));
    runner.refresh();
    let root = temp.path().join("other/application").to_string_lossy().to_string();
    let callback = runner
      .call_on_name("install-dir", |v: &mut WrappedInput| v.set_content(root.clone()))
      .unwrap();
    callback(&mut runner);
    assert_eq!(
      form_values(&mut runner)["bin-dir"],
      temp.path().join("other").to_string_lossy().as_ref()
    );
    runner.user_data::<State>().unwrap().derived_bin = false;
    let custom = temp.path().join("custom-bin").to_string_lossy().to_string();
    runner.call_on_name("bin-dir", |v: &mut WrappedInput| {
      let _ = v.set_content(custom.clone());
    });
    size.store(30, Ordering::SeqCst);
    runner.on_event(Event::WindowResize);
    runner.refresh();
    assert_eq!(form_values(&mut runner)["install-dir"], root);
    size.store(100, Ordering::SeqCst);
    runner.on_event(Event::WindowResize);
    runner.refresh();
    refresh_dependencies(&mut runner);
    assert_eq!(form_values(&mut runner)["bin-dir"], custom);
  }
  #[test]
  fn invalid_values_keep_choices_and_escape_cancels_without_mutation() {
    let (_temp, mut siv, root) = fixture();
    siv.call_on_name("install-dir", |v: &mut WrappedInput| {
      let _ = v.set_content("relative-path");
    });
    start(&mut siv);
    assert_eq!(siv.user_data::<State>().unwrap().stage, Stage::Choices);
    assert!(!root.exists());
    // First dismiss validation, then cancel the choices dialog.
    siv.pop_layer();
    siv.on_event(Event::Key(Key::Esc));
    assert!(siv.user_data::<State>().unwrap().confirming_cancel);
    confirm_cancel(&mut siv);
    assert!(siv
      .user_data::<State>()
      .unwrap()
      .outcome
      .as_ref()
      .unwrap()
      .as_ref()
      .unwrap_err()
      .is::<console::Canceled>());
  }
  #[test]
  fn worker_installs_reports_summary_and_uninstalls_with_temporary_roots() {
    let (_temp, mut siv, root) = fixture();
    let mut runner = siv.runner(backend(Arc::new(AtomicUsize::new(100))));
    runner.refresh();
    start(&mut runner);
    assert_eq!(runner.user_data::<State>().unwrap().stage, Stage::Working);
    let _ = runner.user_data::<State>().unwrap().worker.take().unwrap().join().unwrap();
    runner.process_events();
    assert_eq!(runner.user_data::<State>().unwrap().stage, Stage::Summary);
    assert!(runner.user_data::<State>().unwrap().outcome.as_ref().unwrap().is_ok());
    assert!(!runner.user_data::<State>().unwrap().messages.is_empty());
    assert!(root.join("installer/installation.yaml").is_file());
    drop(runner);
    let package: Package = crate::files::read_yaml(&root.join("installer/installer.yaml")).unwrap();
    let mut uninstall = Cursive::new();
    setup(
      &mut uninstall,
      Action::Uninstall {
        root: root.clone(),
        package,
      },
      Options::default(),
      BTreeMap::new(),
    );
    let mut runner = uninstall.runner(backend(Arc::new(AtomicUsize::new(100))));
    runner.refresh();
    start(&mut runner);
    let _ = runner.user_data::<State>().unwrap().worker.take().unwrap().join().unwrap();
    runner.process_events();
    assert!(runner.user_data::<State>().unwrap().outcome.as_ref().unwrap().is_ok());
    assert!(!root.exists());
  }
  #[test]
  fn cancellation_before_work_does_not_create_an_installation() {
    let (_temp, mut siv, root) = fixture();
    siv.user_data::<State>().unwrap().cancel.store(true, Ordering::SeqCst);
    let mut runner = siv.runner(backend(Arc::new(AtomicUsize::new(100))));
    runner.refresh();
    start(&mut runner);
    let _ = runner.user_data::<State>().unwrap().worker.take().unwrap().join().unwrap();
    runner.process_events();
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
  #[test]
  fn explicit_cli_values_remain_disabled_and_uninstall_settings_are_respected() {
    let (_temp, mut siv, root) = fixture();
    let package = siv.user_data::<State>().unwrap().action.package().clone();
    let options = Options {
      values: BTreeMap::from([("install-dir".into(), root.to_string_lossy().to_string().into())]),
      ..Options::default()
    };
    let mut other = Cursive::new();
    setup(
      &mut other,
      Action::Install {
        config: PathBuf::from("unused"),
        package: package.clone(),
      },
      options.clone(),
      config::default_values(&package, &options).unwrap(),
    );
    assert!(!other
      .call_on_name("enabled-install-dir", |v: &mut EnableableView<BoxedView>| v.is_enabled())
      .unwrap());
    let mut uninstall = Cursive::new();
    setup(
      &mut uninstall,
      Action::Uninstall { root, package },
      Options {
        remove_settings: true,
        settings_explicit: true,
        ..Options::default()
      },
      BTreeMap::new(),
    );
    assert!(uninstall
      .call_on_name("remove-settings", |v: &mut FocusRow| v.is_checked())
      .unwrap());
  }
  #[test]
  fn owned_state_finishes_when_cancellation_arrives_during_activation() {
    let (_temp, mut siv, root) = fixture();
    let action = siv.user_data::<State>().unwrap().action.clone();
    let cancel = Arc::new(AtomicBool::new(false));
    let signal = cancel.clone();
    let operation = Operation {
      cancel,
      report: Arc::new(move |event| {
        if let OperationEvent::Phase(label) = event
          && label.starts_with("Activating")
        {
          signal.store(true, Ordering::SeqCst);
        }
      }),
    };
    let Action::Install { config, package } = action else {
      unreachable!()
    };
    let options = Options {
      interactive: Some(false),
      ..Options::default()
    };
    console::with_operation(operation, || engine::install(&config, package, &options)).unwrap();
    assert!(root.join("installer/installation.yaml").is_file());
    assert!(!root.join("installer/pending.yaml").exists());
    engine::uninstall(&root, &options).unwrap();
  }
  #[test]
  fn ownership_failure_is_shown_in_summary_and_preserves_unowned_files() {
    let (_temp, mut siv, root) = fixture();
    std::fs::create_dir(&root).unwrap();
    std::fs::write(root.join("unowned.txt"), "keep").unwrap();
    let mut runner = siv.runner(backend(Arc::new(AtomicUsize::new(100))));
    runner.refresh();
    start(&mut runner);
    let _ = runner.user_data::<State>().unwrap().worker.take().unwrap().join().unwrap();
    runner.process_events();
    assert_eq!(runner.user_data::<State>().unwrap().stage, Stage::Summary);
    let error = runner
      .user_data::<State>()
      .unwrap()
      .outcome
      .as_ref()
      .unwrap()
      .as_ref()
      .unwrap_err();
    assert!(format!("{error:#}").contains("Choose a fresh install-dir"));
    assert_eq!(std::fs::read_to_string(root.join("unowned.txt")).unwrap(), "keep");
    assert_eq!(std::fs::read_dir(root).unwrap().count(), 1);
  }
  #[test]
  fn undersized_terminal_displays_resize_message_and_blocks_install_button() {
    let (_temp, mut siv, root) = fixture();
    let size = Arc::new(AtomicUsize::new(30));
    let output = Arc::new(std::sync::Mutex::new(String::new()));
    let mut runner = siv.runner(Box::new(Headless {
      size: size.clone(),
      dummy: Dummy,
      output: output.clone(),
    }));
    runner.refresh();
    assert!(output.lock().unwrap().contains("Terminal too small."));
    runner.call_on_name("installer-dialog", |view: &mut Form| {
      let _ = view
        .view
        .set_focus(cursive::views::DialogFocus::Button(if cfg!(windows) { 0 } else { 1 }));
    });
    runner.on_event(Event::Key(Key::Enter));
    assert_eq!(runner.user_data::<State>().unwrap().stage, Stage::Choices);
    assert!(!root.exists());
    size.store(100, Ordering::SeqCst);
    runner.on_event(Event::WindowResize);
    runner.refresh();
    runner.on_event(Event::Key(Key::Enter));
    assert_eq!(runner.user_data::<State>().unwrap().stage, Stage::Working);
    let _ = runner.user_data::<State>().unwrap().worker.take().unwrap().join().unwrap();
    runner.process_events();
    assert!(runner.user_data::<State>().unwrap().outcome.as_ref().unwrap().is_ok());
    engine::uninstall(
      &root,
      &Options {
        interactive: Some(false),
        ..Options::default()
      },
    )
    .unwrap();
  }
  #[test]
  fn cancel_policy_defaults_to_true_and_false_survives_yaml_roundtrip() {
    let (_temp, mut siv, _root) = fixture();
    let package = siv.user_data::<State>().unwrap().action.package().clone();
    assert!(package.runtime.confirm_cancel);
    let mut value = serde_json::to_value(&package).unwrap();
    assert!(value["runtime"].get("confirm-cancel").is_none());
    value["runtime"]["confirm-cancel"] = false.into();
    let package: Package = serde_json::from_value(value.clone()).unwrap();
    let yaml = serde_saphyr::to_string(&package).unwrap();
    assert!(yaml.contains("confirm-cancel: false"));
    assert!(!serde_saphyr::from_str::<Package>(&yaml).unwrap().runtime.confirm_cancel);
    value["runtime"]["confirm-cancel"] = "false".into();
    assert!(serde_json::from_value::<Package>(value).is_err());
  }
  #[test]
  fn escape_prompts_and_enter_defaults_to_no_without_losing_form_values() {
    let (_temp, mut siv, root) = fixture();
    let size = Arc::new(AtomicUsize::new(100));
    let mut runner = siv.runner(backend(size.clone()));
    runner.refresh();
    let values = form_values(&mut runner);
    runner.on_event(Event::Key(Key::Esc));
    runner.refresh();
    assert!(runner.user_data::<State>().unwrap().confirming_cancel);
    assert!(runner.user_data::<State>().unwrap().outcome.is_none());
    runner.on_event(Event::Key(Key::Enter));
    assert!(!runner.user_data::<State>().unwrap().confirming_cancel);
    assert_eq!(form_values(&mut runner), values);
    runner.on_event(Event::CtrlChar('c'));
    runner.refresh();
    assert!(runner.is_running(), "Ctrl+C quit instead of waiting for confirmation");
    assert!(runner.user_data::<State>().unwrap().confirming_cancel);
    runner.on_event(Event::CtrlChar('c'));
    assert!(runner.is_running());
    assert!(!runner.user_data::<State>().unwrap().confirming_cancel);
    assert!(runner.user_data::<State>().unwrap().outcome.is_none());
    runner.on_event(Event::Key(Key::Esc));
    runner.refresh();
    runner.on_event(Event::Key(Key::Esc));
    assert!(!runner.user_data::<State>().unwrap().confirming_cancel);
    assert!(runner.user_data::<State>().unwrap().outcome.is_none());
    runner.on_event(Event::Key(Key::Esc));
    // Confirmation remains usable even when the underlying form cannot fit.
    size.store(30, Ordering::SeqCst);
    runner.on_event(Event::WindowResize);
    runner.refresh();
    runner.on_event(Event::Key(Key::Tab));
    runner.on_event(Event::Key(Key::Enter));
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
  #[test]
  fn disabling_confirmation_cancels_buttons_escape_and_ctrl_c_immediately() {
    for event in [Some(Event::Key(Key::Esc)), Some(Event::CtrlChar('c')), None] {
      let (_temp, mut siv, root) = fixture();
      if let Action::Install { package, .. } = &mut siv.user_data::<State>().unwrap().action {
        package.runtime.confirm_cancel = false;
      }
      if let Some(event) = event {
        siv.on_event(event);
      } else {
        cancel_or_close(&mut siv);
      }
      assert!(!siv.user_data::<State>().unwrap().confirming_cancel);
      assert!(siv
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
  #[test]
  fn worker_completion_dismisses_pending_confirmation_and_close_does_not_prompt() {
    let (_temp, mut siv, _root) = fixture();
    siv.user_data::<State>().unwrap().stage = Stage::Working;
    cancel_or_close(&mut siv);
    assert!(siv.user_data::<State>().unwrap().confirming_cancel);
    assert!(!siv.user_data::<State>().unwrap().cancel.load(Ordering::SeqCst));
    assert_eq!(siv.screen().len(), 2);
    finished(&mut siv, Ok(()));
    assert_eq!(siv.screen().len(), 1);
    assert_eq!(siv.user_data::<State>().unwrap().stage, Stage::Summary);
    assert!(!siv.user_data::<State>().unwrap().confirming_cancel);
    siv.on_event(Event::Key(Key::Esc));
    assert!(siv.user_data::<State>().unwrap().outcome.as_ref().unwrap().is_ok());
    assert_eq!(siv.screen().len(), 1);
  }
  #[test]
  fn external_interrupts_request_confirmation_before_signaling_the_worker() {
    let (_temp, mut siv, _root) = fixture();
    let interrupt = siv.user_data::<State>().unwrap().interrupt.clone();
    let _session = console::TuiSession::begin(interrupt);
    siv.user_data::<State>().unwrap().stage = Stage::Working;
    assert!(console::request_tui_cancel());
    handle_interrupt(&mut siv);
    assert!(siv.user_data::<State>().unwrap().confirming_cancel);
    assert!(!siv.user_data::<State>().unwrap().cancel.load(Ordering::SeqCst));
    dismiss_cancel(&mut siv);
    handle_interrupt(&mut siv);
    assert!(!siv.user_data::<State>().unwrap().confirming_cancel);
    assert!(!siv.user_data::<State>().unwrap().cancel.load(Ordering::SeqCst));
    cancel_or_close(&mut siv);
    confirm_cancel(&mut siv);
    assert!(siv.user_data::<State>().unwrap().cancel.load(Ordering::SeqCst));
  }
}
