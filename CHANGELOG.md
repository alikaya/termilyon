# Changelog

All notable changes to Termilyon are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.13] - 2026-10-10

### Added
- **Move a panel to another tab** — `Shift+Alt+1..9` moves the focused panel to
  tab N, placing it below that tab's most recently active panel, and moves focus
  along with it. If tab N does not exist, the panel is moved into a new tab. The
  source tab is closed when its last panel is moved out. The shortcuts work on
  any keyboard layout, even though `Shift+digit` produces a symbol (e.g. `!`).

## [0.1.12] - 2026-07-01

### Added
- **Directory-aware splits** — a new split panel now starts in the working
  directory of the currently active panel instead of always opening in `$HOME`.
  The directory is resolved from the active panel's foreground process, so it
  follows `cd` correctly and falls back to `$HOME` when it cannot be determined.

### Fixed
- **Reliable new-tab focus** — creating a tab now always switches to and focuses
  the new tab. Previously the wrong page could be selected once some tabs had
  been closed, so the new tab sometimes did not receive focus.
- **Focus after closing a panel** — closing a panel in a split tab now moves
  focus to the most recently active remaining panel in that tab, instead of
  leaving the tab with no focused panel.
- **Crash on closing a split panel** — fixed a `RefCell` double-borrow panic that
  could terminate the whole application when closing a panel (e.g. with `Ctrl+D`)
  in a tab that had multiple panels. The same reentrancy issue on window
  re-activation was hardened as well.

[0.1.13]: https://github.com/alikaya/termilyon/releases/tag/v0.1.13
[0.1.12]: https://github.com/alikaya/termilyon/releases/tag/v0.1.12
