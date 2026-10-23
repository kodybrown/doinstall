# Installer interface decision and console presentation proposal

Date: 2026-10-23. Status: console-only direction selected; presentation changes are proposals, not implemented or approved for implementation.

## Decision to preserve

Keep doinstall's console-only installer and explore polishing the existing Cursive/Crossterm UI. Do not start a desktop GUI implementation merely because a later discussion mentions installers for desktop applications. Review this decision with the user first.

The console installer already works well, including over SSH. Application title, icon, a suitable terminal size, and optionally a larger font may provide enough of the desired standalone-app experience. Missing tools or terminal capabilities are acceptable: unsupported enhancements should be skipped and the existing polished interface should continue normally.

Iced is the preferred candidate if the user later decides a separate graphical installer is justified. It has not been selected for implementation. GUI applications such as Weekend Designer and Weekend Harbor do not by themselves require doinstall to become a desktop GUI.

## Why defer a desktop GUI

- A second interface adds UI, packaging, native startup, accessibility, and platform acceptance work for a small form that already exists in the console.
- C#/Avalonia would introduce another toolchain and runtime distribution plus a Rust/C# integration boundary. A private structured worker protocol was proposed, but the user is concerned about its complexity.
- A Rust-native GUI such as Iced could link the same Rust core directly, avoiding that cross-process protocol. It would still add another graphical UI and renderer to maintain.
- Tauri can link the Rust engine but keeps a webview message bridge and WebView2/WebKit deployment concerns, particularly relevant before an installer can display anything.
- A custom window hosting the TUI would require a terminal renderer/input component and platform PTY support. That may cost more than a small direct Iced interface.
- The expected benefit currently does not justify those additions. Preserve the small console distribution and remote-terminal behavior.

The strongest limitation of this decision is inconsistent outer-window branding across terminals. Accept that limitation instead of growing a terminal-host product to eliminate it.

## Requirements retained if a GUI is reconsidered

Use one shared Rust core for YAML parsing, input resolution/validation, installation, upgrades, settings preservation, receipts, ownership, recovery, cancellation boundaries, shortcuts, commands, PATH, pruning, and uninstall. Do not implement a parallel C# or web configuration resolver.

Console and GUI would be separate generated entry points, selected at build time by one setting such as builder `--gui`, with the exact same installer.yaml schema and behavior. No runtime console/GUI switch is wanted. Directory and self-extracting delivery are independent choices. Prebuilt target components should be supplied by doinstall; consumer projects should not compile installer templates. A graphical package must also retain a graphical uninstaller and handle pre-UI extraction errors. None of these proposed GUI flags or components exist as a result of this discussion.

## Console layout proposal

Refine the current visual language rather than redesign it: dark navy backdrop, slate form surface, darker editable fields, pale text, subdued captions, cyan focus, filled action buttons, color-defined edges and small shadows. Preserve the user's preference for no box-drawing borders. Keep application-defined YAML wording authoritative.

- Review an 80-column by 24-row terminal as the compact baseline, rather than forcing every terminal to that size. Retain responsive growth, long-path wrapping, scrolling, minimum-size notices, and all field values during resize.
- Choices: application name/version and description, `Install to:` field, existing relevant options, optional application inputs, then keyboard hints and Install/Cancel actions. Keep current YAML defaults, locked values, dependencies, and platform button order.
- Working: application identity, truthful current operation text and diagnostic details; preserve existing cancellation semantics. No fake percentage, progress bar, or spinner.
- Summary: clear actual outcome, destination and useful messages, then Close. Do not imply rollback or cancellation after the operation has completed.
- Font enlargement is a bonus where supported; do not require a specific font, install fonts, or shrink text to conceal layout problems.

Visual concepts shown with this discussion are illustrative layout proposals, not screenshots of an implemented installer, exact character-grid validation, or evidence of native host support. Existing source and presentation rules were inspected; production installer code is unchanged.

## Host detection and behavior proposal

Detect terminal capabilities, not just the shell name. PowerShell, CMD, Bash, and zsh can run inside many terminal hosts. Environment hints such as WT_SESSION, TERM_PROGRAM and TERM, terminal status, native window information, and bounded capability checks can inform a small supported-host list. Hints may be inherited or obscured by SSH, tmux, screen, and IDE terminals; TERM=xterm-256color is not proof of xterm.

Apply supported temporary presentation changes early, before drawing the form. They need not all be supplied as shell command-line arguments. Skip unknown or unavailable capabilities without failing installation. Do not assume API success proves a visible terminal changed; validate native behavior per host. Persistent terminal preferences and profiles remain user-owned.

| Host | Title after startup | Size after startup | Icon after startup | Font size after startup |
| --- | --- | --- | --- | --- |
| Classic Windows Console Host | Supported through console APIs | Supported subject to buffer, font, and screen limits | Candidate Win32 window-icon change when the visible console window is confirmed | Supported by legacy console font API; native acceptance still needed |
| Windows Terminal | Application title request, subject to host policy | Do not promise classic APIs resize the visible window; launch-time --size is documented | Profile tab icon is supported; no general app-controlled replacement is assumed | Host setting/action; no general child-app API is assumed |
| xterm | Supported control sequence | Resize request when host policy permits | Icon label is not an image; no portable image-icon change assumed | Font control exists but depends on configuration/font support |
| macOS Terminal / iTerm2 | Supported title requests, subject to settings | Host-specific controls/automation; verify per host | No portable image-icon change assumed | Profiles/automation or supported host-specific facilities; verify before adding |
| kitty | Supported host controls | Host-specific facilities | No portable OS-icon replacement assumed | Remote-control command exists when access is enabled; scope can include the OS window |
| Unknown, multiplexed, SSH, redirected | Use only established safe capabilities; skip unsupported behavior | Preserve current size unless reliably supported | Preserve existing icon | Preserve existing font |

The downloaded installer's file icon, a terminal tab icon, a window icon, and an icon *label* are distinct features. Embedding an EXE icon does not guarantee Windows Terminal adopts it. Live terminal title strings must be sanitized before emitting control sequences.

For a reused terminal, prefer reversible session changes and restore captured state where supported. Avoid shrinking a user's larger terminal and avoid resetting dimensions/font after the user manually changes them. Apply size preferences to a dedicated launch window only where that scope can be established. Do not target the foreground window, rewrite settings.json, or simulate keyboard shortcuts to force unsupported presentation. These are proposed implementation safeguards, not implemented features.

## Sources checked on 2026-10-23

- [Windows Terminal command-line options](https://learn.microsoft.com/en-us/windows/terminal/command-line-arguments): title, new-window selection and initial --size.
- [Windows Terminal profiles](https://learn.microsoft.com/en-us/windows/terminal/customize-settings/profile-general): tab icon and title settings.
- [GetConsoleWindow](https://learn.microsoft.com/en-us/windows/console/getconsolewindow): pseudoconsole handles do not identify the visible terminal window.
- [SetConsoleWindowInfo](https://learn.microsoft.com/en-us/windows/console/setconsolewindowinfo) and [console sizing](https://learn.microsoft.com/en-us/windows/console/window-and-screen-buffer-size): classic console size control and limits.
- [SetCurrentConsoleFontEx](https://learn.microsoft.com/en-us/windows/console/setcurrentconsolefontex): legacy font control with no virtual-terminal equivalent.
- [WM_SETICON](https://learn.microsoft.com/en-us/windows/win32/winmsg/wm-seticon): window-icon control, not a Windows Terminal tab API.
- [Xterm control sequences](https://invisible-island.net/xterm/ctlseqs/ctlseqs.html): titles, window operations and configurable font controls.
- [iTerm2 escape codes](https://iterm2.com/documentation-escape-codes.html): session profile switching and host-specific features.
- [Kitty remote control](https://sw.kovidgoyal.net/kitty/remote-control/): host commands, access requirements and font scope.

## Verification and next acceptance gate

This task saves a decision and design proposal only. Review the documentation diff, links, encoding/EOL preservation and Git whitespace; runtime tests are not warranted for a documentation-only change. Native title/icon/size/font support is research-derived and has not been exercised on Windows, Linux, macOS or SSH in this task. Before implementation, the user should select or refine the visual proposal and agree on the first supported-host scope.
