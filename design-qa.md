# Console presentation design QA

Date: 2026-10-23. Scope: the production Rust/Cursive console interface.

## Visual reference and evidence

The third displayed concept remains the styling reference. The user's subsequent
two screenshots refine sizing: remove the extra strip above the dialog title,
fill the terminal viewport, align controls at the top, stretch editors across the
usable width, and keep actions at the bottom. This supersedes the earlier
content-height main dialog requirement. Confirmation and validation overlays
remain compact.

Actual Cursive backend cells/colors were captured at 80x24 and 100x32, including
bold effects, then rendered with 10x22 pixel cells into local ignored artifacts:

- `artifacts/console-presentation/selected-layout-80x24.png` (800x528).
- `artifacts/console-presentation/selected-layout-100x32.png` (1000x704).

Both captures use `C:\Users\User\Apps\WeekendDesigner` and actual controls with
the installation-path editor focused. They are code-rendered character cells,
not native OS-window screenshots. The earlier `comparison.png` and
`selected-reference.png` document the original concept only; they are superseded
for main-screen sizing by this screenshot review.

## Comparison and findings

The rendered views were inspected against the supplied mockup. A single flat
header spans the viewport; its one padded row is continuous, with no reserved
border strip or indented header edge. Title and body controls share the same
horizontal inset. The focused editor's background fills the usable width.
Controls stay near the top, the body takes remaining height, and actions and
keyboard hints remain anchored at the bottom. Existing slate/navy colors, cyan
primary action and field focus are retained.

No actionable visual discrepancy remains within this console layout scope.
Package-defined labels, versions and controls remain authoritative. Font metrics
and graphical terminal chrome are host-owned. Flat cell fills replace gradients.

## Interaction and verification

- Main-screen geometry is asserted at both 100x32 and 80x24, including header
  position/edge colors, editor width, bottom actions and last-row hints.
- Removing Cursive's reserved top/side border cells translates drawing, layout,
  pointer events and focus rectangles together. Existing real mouse-hit, keyboard,
  checkbox wrapping, Unicode editing and cursor-preservation tests pass.
- Compact confirmation retains No as its default and working Yes hit targets.
  Minimum-size notices, validation, scrolling, and cancellation remain covered.
- Windows `build.cmd verify` passes 158 workspace tests, formatting, strict Clippy,
  shim integrity and whitespace checks. Native Debian WSL passes 42 installer
  unit tests, seven terminal protocol scenarios in one PTY acceptance test, and
  all-target strict Clippy. Cargo outputs remain external and checkout-specific.

## Acceptance limits

These captures establish rendered console layout, not physical host acceptance
for classic Windows icon/font APIs, Windows Terminal/iTerm2 chrome, or native
macOS execution. No terminal capability behavior changed in this follow-up.

final result: passed
