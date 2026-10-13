//! Fixed presentation rules over Cursive's controls and event handling.
use super::{refresh_dependencies, State};
use cursive::{
  event::{Callback, Event, EventResult, Key, MouseButton, MouseEvent},
  theme::{BorderStyle, Color, ColorStyle, Effect, PaletteColor, PaletteStyle, Style, Theme},
  view::{View, ViewWrapper},
  views::{BoxedView, Dialog, DummyView, PaddedView, TextArea, ThemedView},
  Printer, Vec2,
};

pub(super) const SURFACE: Color = Color::Rgb(35, 43, 55);
const EDGE: Color = Color::Rgb(48, 63, 82);
const TEXT: Color = Color::Rgb(235, 241, 248);
const MUTED: Color = Color::Rgb(157, 174, 195);
const FIELD: Color = Color::Rgb(24, 32, 44);
const FOCUS: Color = Color::Rgb(119, 205, 236);
const BUTTON: Color = Color::Rgb(57, 90, 128);
pub(super) const WARNING: Color = Color::Rgb(255, 203, 112);
pub(super) const ERROR: Color = Color::Rgb(255, 143, 151);
pub(super) const SUCCESS: Color = Color::Rgb(139, 222, 177);

pub(super) fn theme() -> Theme {
  let mut theme = Theme {
    shadow: true,
    borders: BorderStyle::None,
    ..Theme::default()
  };
  for (role, color) in [
    (PaletteColor::Background, Color::Rgb(15, 21, 30)),
    (PaletteColor::Shadow, Color::Rgb(5, 9, 15)),
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

/// The dialog still owns layout, focus and button hit testing. Only its title
/// and frame are painted here; padded raw labels remain ordinary Buttons.
pub(super) struct Form {
  pub(super) view: Dialog,
  title: String,
}
impl Form {
  pub(super) fn new(mut view: Dialog) -> Self {
    let title = view.get_title().to_owned();
    // Stock titles draw separator glyphs even with BorderStyle::None.
    view.set_title("");
    view.set_padding(cursive::view::Margins::lrtb(2, 2, 1, 1));
    for button in view.buttons_mut() {
      let label = button.label().trim_start_matches('<').trim_end_matches('>').to_owned();
      button.set_label_raw(format!(" {label} "));
    }
    // Give the body its ordinary text palette while the dialog's button row
    // uses filled Primary/Highlight styles. No button geometry is duplicated.
    let content = view.set_content(DummyView);
    view.set_content(ThemedView::new(theme(), BoxedView::new(content)));
    Self { view, title }
  }
}
impl ViewWrapper for Form {
  cursive::wrap_impl!(self.view: Dialog);
  fn wrap_draw(&self, printer: &Printer) {
    printer.with_color(ColorStyle::new(TEXT, EDGE), |p| {
      if self.view.buttons().next().is_some() {
        for y in p.size.y.saturating_sub(4)..p.size.y {
          p.print_hline((0, y), p.size.x, " ");
        }
      }
    });
    let mut buttons = theme();
    buttons.palette[PaletteStyle::Primary] = ColorStyle::new(TEXT, BUTTON).into();
    self.view.draw(&printer.theme(&buttons));
    if printer.size.x == 0 || printer.size.y == 0 {
      return;
    }
    printer.with_color(ColorStyle::new(TEXT, EDGE), |p| {
      p.print_hline((0, 0), p.size.x, " ");
      p.print_hline((0, p.size.y - 1), p.size.x, " ");
      p.print_vline((0, 0), p.size.y, " ");
      p.print_vline((p.size.x - 1, 0), p.size.y, " ");
      p.with_effect(Effect::Bold, |p| p.print((2, 0), &self.title));
    });
  }
}

/// Reserve a marker column and highlight the complete labelled checkbox. The
/// wrapper forwards focus, Space/Enter and mouse handling to the stock control.
pub(super) struct FocusRow<V> {
  view: PaddedView<V>,
  size: Vec2,
}
impl<V> FocusRow<V> {
  pub(super) fn new(view: V) -> Self {
    Self {
      view: PaddedView::lrtb(2, 0, 0, 0, view),
      size: Vec2::zero(),
    }
  }
}
impl<V: View> ViewWrapper for FocusRow<V> {
  cursive::wrap_impl!(self.view: PaddedView<V>);
  fn wrap_layout(&mut self, size: Vec2) {
    self.size = size;
    self.view.layout(size);
  }
  fn wrap_on_event(&mut self, event: Event) -> EventResult {
    if let Event::Mouse {
      event: MouseEvent::Release(MouseButton::Left),
      position,
      offset,
    } = event
    {
      // Make the whole labelled row a checkbox target, including the focus
      // marker; EnableableView still refuses toggles for locked options.
      if position.fits_in_rect(offset, self.size) {
        return self.view.on_event(Event::Char(' '));
      }
    }
    self.view.on_event(event)
  }
  fn wrap_draw(&self, printer: &Printer) {
    let focused = printer.focused && printer.enabled;
    printer.with_selection(focused, |p| {
      p.print_hline((0, 0), p.size.x, " ");
      p.print((0, 0), if focused { ">" } else { " " });
      self.view.draw(p);
    });
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
      theme.palette[PaletteStyle::EditableText] = ColorStyle::new(FIELD, FOCUS).into();
      theme.palette[PaletteStyle::EditableTextCursor] = ColorStyle::new(TEXT, BUTTON).into();
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
