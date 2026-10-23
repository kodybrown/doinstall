//! Fixed presentation rules over Cursive's controls and event handling.
use super::{refresh_dependencies, State};
use cursive::{
  event::{Callback, Event, EventResult, Key, MouseButton, MouseEvent},
  theme::{BorderStyle, Color, ColorStyle, Effect, PaletteColor, PaletteStyle, Style, Theme},
  utils::markup::StyledString,
  view::{Scrollable, View, ViewWrapper},
  views::{
    BoxedView, Checkbox, Dialog, DummyView, LinearLayout, PaddedView, SelectView, TextArea, TextView, ThemedView,
  },
  Printer, Vec2,
};
use unicode_width::UnicodeWidthStr;

pub(super) const SURFACE: Color = Color::Rgb(35, 43, 55);
const EDGE: Color = Color::Rgb(48, 63, 82);
const TEXT: Color = Color::Rgb(235, 241, 248);
const MUTED: Color = Color::Rgb(157, 174, 195);
const FIELD: Color = Color::Rgb(24, 32, 44);
const FOCUS: Color = Color::Rgb(119, 205, 236);
const BUTTON: Color = Color::Rgb(57, 90, 128);
const BACKDROP: Color = Color::Rgb(15, 21, 30);
const SHADOW: Color = Color::Rgb(5, 9, 15);
const FIELD_FOCUS: Color = Color::Rgb(20, 44, 65);
pub(super) const WARNING: Color = Color::Rgb(255, 203, 112);
pub(super) const ERROR: Color = Color::Rgb(255, 143, 151);
pub(super) const SUCCESS: Color = Color::Rgb(139, 222, 177);

pub(super) fn notice_style() -> ColorStyle {
  ColorStyle::new(TEXT, BACKDROP)
}

pub(super) fn description(text: impl Into<String>) -> StyledString {
  StyledString::styled(text.into(), ColorStyle::new(MUTED, SURFACE))
}

pub(super) fn theme() -> Theme {
  let mut theme = Theme {
    // Flat cell surfaces match the compact console design. The terminal owns
    // its outer window; Cursive must not add a stock frame or shadow.
    shadow: false,
    borders: BorderStyle::None,
    ..Theme::default()
  };
  for (role, color) in [
    (PaletteColor::Background, BACKDROP),
    (PaletteColor::Shadow, SHADOW),
    (PaletteColor::View, SURFACE),
    (PaletteColor::Primary, TEXT),
    (PaletteColor::Secondary, MUTED),
    (PaletteColor::Tertiary, MUTED),
    (PaletteColor::TitlePrimary, TEXT),
    (PaletteColor::TitleSecondary, FOCUS),
    (PaletteColor::Highlight, FOCUS),
    (PaletteColor::HighlightInactive, BUTTON),
    (PaletteColor::HighlightText, FIELD),
  ] {
    theme.palette[role] = color;
  }
  theme.palette[PaletteStyle::Primary] = ColorStyle::new(TEXT, SURFACE).into();
  theme.palette[PaletteStyle::Secondary] = ColorStyle::new(MUTED, SURFACE).into();
  // Explicit styles avoid the retro palette's reverse-video effects, so both
  // rendering and color-only terminals use the same foreground/background pair.
  theme.palette[PaletteStyle::Highlight] = Style::from(ColorStyle::new(FIELD, FOCUS)).combine(Effect::Bold);
  theme.palette[PaletteStyle::HighlightInactive] = ColorStyle::new(TEXT, BUTTON).into();
  theme.palette[PaletteStyle::EditableText] = ColorStyle::new(TEXT, FIELD).into();
  theme.palette[PaletteStyle::EditableTextInactive] = ColorStyle::new(MUTED, SURFACE).into();
  theme.palette[PaletteStyle::EditableTextCursor] = ColorStyle::new(FIELD, FOCUS).into();
  theme
}

/// A flat header inside the scrolling content: long application descriptions
/// remain readable without taking space away from the form's action buttons.
struct Header {
  view: PaddedView<LinearLayout>,
}
impl Header {
  fn new(title: &str, description: &str) -> Self {
    let mut content = LinearLayout::vertical().child(TextView::new(StyledString::styled(
      title,
      Style::from(ColorStyle::new(TEXT, EDGE)).combine(Effect::Bold),
    )));
    if !description.is_empty() {
      content.add_child(TextView::new(StyledString::styled(description, ColorStyle::new(MUTED, EDGE))));
    }
    Self {
      view: PaddedView::lrtb(2, 2, 1, 1, content),
    }
  }
}
impl ViewWrapper for Header {
  cursive::wrap_impl!(self.view: PaddedView<LinearLayout>);
  fn wrap_draw(&self, printer: &Printer) {
    printer.with_color(ColorStyle::new(TEXT, EDGE), |p| {
      for y in 0..p.size.y {
        p.print_hline((0, y), p.size.x, " ");
      }
      self.view.draw(p);
    });
  }
}

/// Cursive still owns layout, editing, focus, buttons and mouse hit targets.
/// Header and body share a scroller; the footer stays put and the keyboard hints
/// live on the backdrop below it, as in the selected console design.
pub(super) struct Form {
  pub(super) view: Dialog,
  title: String,
  instructions: bool,
  cycle_buttons: bool,
}
impl Form {
  pub(super) fn new(view: Dialog) -> Self {
    Self::with_description(view, "")
  }
  pub(super) fn with_description(mut view: Dialog, description: &str) -> Self {
    let title = view.get_title().to_owned();
    // Suppress stock title separators. Borders remain space/color surfaces,
    // never box-drawing characters or a second set of interactive controls.
    view.set_title("");
    view.set_padding(cursive::view::Margins::lrtb(0, 0, 0, 0));
    let content = view.set_content(DummyView);
    view.set_content(ThemedView::new(
      theme(),
      LinearLayout::vertical()
        .child(Header::new(&title, description))
        .child(PaddedView::lrtb(2, 2, 1, 1, BoxedView::new(content)))
        .scrollable(),
    ));
    let mut form = Self {
      view,
      title,
      instructions: false,
      cycle_buttons: false,
    };
    form.refresh_buttons();
    form
  }
  pub(super) fn popup(mut view: Dialog) -> Self {
    // Preserve a blank surface row around modal body text.
    let content = view.set_content(DummyView);
    view.set_content(PaddedView::lrtb(0, 0, 1, 1, BoxedView::new(content)));
    let mut form = Self::new(view);
    form.cycle_buttons = form.title == "Confirm";
    form
  }
  pub(super) fn instructions(mut self) -> Self {
    self.instructions = true;
    self
  }
  fn refresh_buttons(&mut self) {
    let focus = self.view.focus();
    for (index, button) in self.view.buttons_mut().enumerate() {
      let label = button.label().trim().trim_start_matches('<').trim_end_matches('>').to_owned();
      let primary = matches!(label.as_str(), "Install" | "Uninstall" | "Close" | "OK");
      let text = format!(" {label} ");
      // Keep the action identifiable while a field is focused. Focused buttons
      // use the parent highlight instead, so keyboard focus stays unmistakable.
      if primary && focus != cursive::views::DialogFocus::Button(index) {
        button.set_label_raw(StyledString::styled(text, ColorStyle::new(FIELD, FOCUS)));
      } else {
        button.set_label_raw(text);
      }
    }
  }
}
impl ViewWrapper for Form {
  cursive::wrap_impl!(self.view: Dialog);
  fn wrap_required_size(&mut self, available: Vec2) -> Vec2 {
    let hints = usize::from(self.instructions);
    let usable = available.saturating_sub((0, hints));
    let mut size = self.view.required_size(usable);
    size.x = size.x.max(48).min(usable.x);
    size + (0, hints)
  }
  fn wrap_layout(&mut self, size: Vec2) {
    self.view.layout(size.saturating_sub((0, usize::from(self.instructions))));
    self.refresh_buttons();
  }
  fn wrap_on_event(&mut self, event: Event) -> EventResult {
    // A wrapped header makes the content scrollable at narrow sizes. Keep Tab
    // between the two confirmation choices rather than focusing that scroller
    // between No and Yes. Enter must always activate the visibly focused choice.
    let result = if self.cycle_buttons
      && self.view.buttons_len() == 2
      && matches!(event, Event::Key(Key::Tab) | Event::Shift(Key::Tab))
      && let cursive::views::DialogFocus::Button(index) = self.view.focus()
    {
      self
        .view
        .set_focus(cursive::views::DialogFocus::Button(1 - index))
        .and(EventResult::consumed())
    } else {
      self.view.on_event(event)
    };
    self.refresh_buttons();
    result
  }
  fn wrap_important_area(&self, size: Vec2) -> cursive::Rect {
    self
      .view
      .important_area(size.saturating_sub((0, usize::from(self.instructions))))
  }
  fn wrap_draw(&self, printer: &Printer) {
    let size = printer.size.saturating_sub((0, usize::from(self.instructions)));
    if size.x < 2 || size.y < 4 {
      return;
    }
    let form = printer.cropped(size);
    form.with_color(ColorStyle::new(TEXT, SURFACE), |p| {
      for y in 0..size.y {
        p.print_hline((0, y), size.x, " ");
      }
    });
    // The dialog reserves one border row above content and three action rows.
    // Color them without repainting over any of the stock controls.
    form.with_color(ColorStyle::new(TEXT, EDGE), |p| {
      p.print_hline((0, 0), size.x, " ");
      for y in size.y.saturating_sub(3)..size.y {
        p.print_hline((0, y), size.x, " ");
      }
    });
    let mut buttons = theme();
    buttons.palette[PaletteStyle::Primary] = ColorStyle::new(TEXT, BUTTON).into();
    buttons.palette[PaletteStyle::Highlight] = Style::from(ColorStyle::new(FIELD, TEXT)).combine(Effect::Bold);
    self.view.draw(&form.theme(&buttons));
    if self.instructions {
      let hint = [
        "Tab: move  Space: toggle  Esc: cancel",
        "Tab: move  Esc: cancel",
        "Esc: cancel",
      ]
      .into_iter()
      .find(|hint| hint.width() <= size.x.saturating_sub(6))
      .unwrap_or("");
      printer.with_color(ColorStyle::new(MUTED, BACKDROP), |p| {
        p.print_hline((0, size.y), size.x, " ");
        p.print((3, size.y), hint);
      });
    }
  }
}

/// Wrap the label beside the stock checkbox, highlighting every continuation
/// row and keeping the entire option clickable. Stored values never change.
pub(super) struct FocusRow {
  view: Checkbox,
  label: TextView,
  size: Vec2,
}
impl FocusRow {
  pub(super) fn new(view: Checkbox, label: impl Into<String>) -> Self {
    Self {
      view,
      label: TextView::new(label.into()),
      size: Vec2::zero(),
    }
  }
  pub(super) fn is_checked(&self) -> bool {
    self.view.is_checked()
  }
  pub(super) fn set_checked(&mut self, checked: bool) -> EventResult {
    self.view.set_checked(checked)
  }
  pub(super) fn is_enabled(&self) -> bool {
    self.view.is_enabled()
  }
  pub(super) fn set_enabled(&mut self, enabled: bool) {
    self.view.set_enabled(enabled);
  }
}
impl ViewWrapper for FocusRow {
  cursive::wrap_impl!(self.view: Checkbox);
  fn wrap_required_size(&mut self, available: Vec2) -> Vec2 {
    let label = self.label.required_size(Vec2::new(available.x.saturating_sub(4), usize::MAX));
    Vec2::new(label.x + 4, label.y.max(1))
  }
  fn wrap_layout(&mut self, size: Vec2) {
    self.size = size;
    self.view.layout(Vec2::new(3, 1));
    self.label.layout(size.saturating_sub((4, 0)));
  }
  fn wrap_important_area(&self, _: Vec2) -> cursive::Rect {
    cursive::Rect::from_size(Vec2::zero(), self.size)
  }
  fn wrap_on_event(&mut self, event: Event) -> EventResult {
    if let Event::Mouse {
      event: MouseEvent::Release(MouseButton::Left),
      position,
      offset,
    } = event
    {
      // Continuation lines also toggle the option; the stock checkbox still
      // refuses changes and focus for locked options.
      if position.fits_in_rect(offset, self.size) {
        return self.view.on_event(Event::Char(' '));
      }
    }
    self.view.on_event(event)
  }
  fn wrap_draw(&self, printer: &Printer) {
    let enabled = printer.enabled && self.is_enabled();
    let focused = printer.focused && enabled;
    let mut theme = theme();
    // A darker accent preserves contrast for the requested white checkbox
    // labels. Focus is expressed by the entire row, never a separate marker.
    theme.palette[PaletteStyle::Highlight] = Style::from(ColorStyle::new(TEXT, BUTTON)).combine(Effect::Bold);
    let printer = printer.theme(&theme);
    let colors = ColorStyle::new(if enabled { TEXT } else { MUTED }, if focused { BUTTON } else { SURFACE });
    printer.with_color(colors, |p| {
      for y in 0..p.size.y {
        p.print_hline((0, y), p.size.x, " ");
      }
      self.view.draw(p);
      self.label.draw(&p.offset((4, 0)));
    });
  }
}

/// Preserve SelectView's item navigation and values while mapping each logical
/// item to wrapped display rows. Mouse events and scroll focus use that mapping.
pub(super) struct WrappedChoices {
  view: SelectView<String>,
  labels: Vec<TextView>,
  rows: Vec<(usize, usize)>,
  size: Vec2,
}
impl WrappedChoices {
  pub(super) fn new(view: SelectView<String>) -> Self {
    let labels = view.iter().map(|(label, _)| TextView::new(label)).collect();
    Self {
      view,
      labels,
      rows: Vec::new(),
      size: Vec2::zero(),
    }
  }
  pub(super) fn selection(&self) -> Option<std::sync::Arc<String>> {
    self.view.selection()
  }
  pub(super) fn set_enabled(&mut self, enabled: bool) {
    self.view.set_enabled(enabled);
  }
}
impl ViewWrapper for WrappedChoices {
  cursive::wrap_impl!(self.view: SelectView<String>);
  fn wrap_required_size(&mut self, available: Vec2) -> Vec2 {
    let height = self
      .labels
      .iter_mut()
      .map(|label| label.required_size(Vec2::new(available.x, usize::MAX)).y.max(1))
      .sum();
    Vec2::new(available.x, height)
  }
  fn wrap_layout(&mut self, size: Vec2) {
    self.size = size;
    self.rows.clear();
    let mut y = 0;
    for label in &mut self.labels {
      let height = label.required_size(Vec2::new(size.x, usize::MAX)).y.max(1);
      label.layout(Vec2::new(size.x, height));
      self.rows.push((y, height));
      y += height;
    }
    self.view.layout(Vec2::new(size.x, self.labels.len()));
  }
  fn wrap_important_area(&self, size: Vec2) -> cursive::Rect {
    self
      .view
      .selected_id()
      .and_then(|i| self.rows.get(i))
      .map(|&(y, height)| cursive::Rect::from_size((0, y), (size.x, height)))
      .unwrap_or_else(|| cursive::Rect::from_size(Vec2::zero(), size))
  }
  fn wrap_on_event(&mut self, event: Event) -> EventResult {
    if let Event::Mouse {
      event,
      position,
      offset,
    } = event
    {
      if let Some(position) = position.checked_sub(offset).filter(|p| p.fits_in(self.size))
        && let Some(i) = self
          .rows
          .iter()
          .position(|&(y, height)| position.y >= y && position.y < y + height)
      {
        return self.view.on_event(Event::Mouse {
          event,
          position: Vec2::new(position.x, i),
          offset: Vec2::zero(),
        });
      }
      return EventResult::Ignored;
    }
    self.view.on_event(event)
  }
  fn wrap_draw(&self, printer: &Printer) {
    let enabled = printer.enabled && self.view.is_enabled();
    for (i, (label, &(y, height))) in self.labels.iter().zip(&self.rows).enumerate() {
      let p = printer.offset((0, y)).cropped((printer.size.x, height));
      let selected = enabled && self.view.selected_id() == Some(i);
      let style = if !enabled {
        PaletteStyle::Secondary
      } else if selected && printer.focused {
        PaletteStyle::Highlight
      } else if selected {
        PaletteStyle::HighlightInactive
      } else {
        PaletteStyle::Primary
      };
      p.with_style(style, |p| {
        for y in 0..p.size.y {
          p.print_hline((0, y), p.size.x, " ");
        }
        label.draw(p);
      });
    }
  }
}

/// A soft-wrapped single value, using TextArea's Unicode editing and cursor
/// navigation. Wrapping never inserts line breaks into installation paths.
pub(super) struct WrappedInput {
  view: TextArea,
  key: String,
  changed: bool,
  #[cfg(test)]
  pub(super) size: Vec2,
}
impl WrappedInput {
  pub(super) fn new(key: String, content: String) -> Self {
    let end = content.len();
    let mut view = TextArea::new().content(content);
    view.set_cursor(end);
    Self {
      view,
      key,
      changed: true,
      #[cfg(test)]
      size: Vec2::zero(),
    }
  }
  pub(super) fn get_content(&self) -> &str {
    self.view.get_content()
  }
  pub(super) fn set_enabled(&mut self, enabled: bool) {
    if enabled {
      self.view.enable();
    } else {
      self.view.disable();
    }
  }
  #[cfg(test)]
  pub(super) fn get_cursor(&self) -> usize {
    self.view.cursor()
  }
  pub(super) fn set_content(&mut self, content: impl Into<String>) -> Callback {
    self.view.set_content(content);
    self.changed = true;
    self.callback()
  }
  fn callback(&self) -> Callback {
    let key = self.key.clone();
    Callback::from_fn(move |s| {
      if key == "bin-dir" {
        s.user_data::<State>().unwrap().derived_bin = false;
      }
      refresh_dependencies(s);
    })
  }
}
impl ViewWrapper for WrappedInput {
  cursive::wrap_impl!(self.view: TextArea);
  fn wrap_required_size(&mut self, available: Vec2) -> Vec2 {
    let size = self.view.required_size(Vec2::new(available.x, usize::MAX));
    // Bound very long values; the stock editor scrolls to keep its cursor in view.
    Vec2::new(available.x, size.y.clamp(1, 4))
  }
  fn wrap_layout(&mut self, size: Vec2) {
    #[cfg(test)]
    {
      self.size = size;
    }
    self.view.layout(size);
    self.changed = false;
  }
  fn wrap_needs_relayout(&self) -> bool {
    self.changed || self.view.needs_relayout()
  }
  fn wrap_draw(&self, printer: &Printer) {
    let mut theme = theme();
    if printer.focused && printer.enabled {
      theme.palette[PaletteStyle::EditableText] = ColorStyle::new(FOCUS, FIELD_FOCUS).into();
      theme.palette[PaletteStyle::EditableTextCursor] = ColorStyle::new(FIELD_FOCUS, FOCUS).into();
    }
    self.view.draw(&printer.theme(&theme));
  }
  fn wrap_on_event(&mut self, event: Event) -> EventResult {
    match event {
      Event::Key(Key::Enter) | Event::Char('\n' | '\r' | '\t') => return EventResult::consumed(),
      Event::Key(Key::Tab) | Event::Shift(Key::Tab) => return EventResult::Ignored,
      _ => {}
    }
    let before = self.view.get_content().to_owned();
    let result = self.view.on_event(event);
    if self.view.get_content() != before {
      self.changed = true;
      result.and(EventResult::Consumed(Some(self.callback())))
    } else {
      result
    }
  }
}
