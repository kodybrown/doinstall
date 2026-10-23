# Console presentation design QA

Date: 2026-10-23. Scope: the production Rust/Cursive console interface, not a web prototype.

## Source and implementation evidence

- Source visual truth: the third displayed concept selected by the user, with
  the requested User-only path correction, saved as
  `artifacts/console-presentation/selected-reference.png` (1586x992 pixels).
- Implementation: real Cursive backend cells/colors at 80x24, rendered with a
  10x22 pixel cell into `artifacts/console-presentation/selected-layout-80x24.png`
  (800x528), and 100x32 into `selected-layout-100x32.png` (1000x704).
- Full-view comparison: `artifacts/console-presentation/comparison.png` puts both
  artifacts together. The illustrative reference OS title bar is excluded; the
  remaining source region is uniformly scaled to 800 pixels wide and letterboxed.
  Implementation cells retain their recorded resolution. No browser/CSS viewport
  or deviceScaleFactor applies to this native terminal interface.
- State: choices with the installation-path field focused. Sample paths are
  `C:\Users\User\Apps\WeekendDesigner`. Captures show actual field/control state;
  they are not native OS-window screenshots and do not prove icon/font support.
- Artifacts are local ignored review output and are not installer payload assets.

## Comparison and findings

No actionable P0/P1/P2 differences remain within the approved console scope.
The header/subtitle hierarchy, navy/slate surfaces, dark cyan-text field focus,
filled primary action, secondary action and separate backdrop hints match the
selected direction using terminal cells. Captions and controls remain aligned.

The user explicitly retained current sizing behavior. Accordingly, the form
continues filling the terminal's usable width and grows vertically with content,
rather than imposing the generated illustration's fixed-looking panel ratio.
The real fixture retains current version metadata, package-driven permissions,
wording and dependent command/PATH fields; the illustration's different option
inventory is not treated as a configuration change. Flat fills replace generated
gradients, and graphical OS chrome remains owned by the terminal host.

Focused header/field/footer regions were inspected in the combined full-view
image; separate region crops were unnecessary for this small interface. Production
bold effects are also recorded by the backend capture for the final previews.

## Interaction and responsive verification

- Real controls preserve click targets, Tab/Shift+Tab, checkbox continuation-row
  focus/clicks, disabled-field skipping, Unicode editing and cursor preservation.
- At 80x24 actions/hints remain visible; larger descriptions increase required
  dialog height. Small-terminal notices retain values and do not start install.
- Cancellation stays No by default. A test caught an extra scroll-focus stop in
  narrow confirmation dialogs; cycling the two choices and consuming the focus
  event fixed it. Windows and Linux regressions then passed.
- Windows full verification passed 158 tests. Linux passed 42 installer units,
  seven PTY scenarios in one acceptance test, repeated PTY stability checks and
  strict Clippy. Validation/unattended streams remain plain and presentation
  preferences do not become installation values or receipt settings.

## Acceptance limits

Physical classic Console Host icon/font behavior, native Windows Terminal and
iTerm2 chrome, and native macOS execution require host-specific acceptance.
PTY emulation verifies emitted protocols and fallback behavior, not that every
graphical host honors them. No universal icon/font/resize claim is made.

## Review history

The main visual comparison found no implementation P0/P1/P2 issue. Differences
in terminal aspect, option inventory and rasterized gradients were classified
against the user's explicit terminal/sizing/configuration constraints, not fixed
by changing installation behavior. Functional focus regressions were resolved
before the final rendered evidence. The preview recorder was extended to retain
the real bold effects rather than infer heading weight from colors.

final result: passed
