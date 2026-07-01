# Changelog

All notable changes to Termilyon are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

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

[0.1.12]: https://github.com/alikaya/termilyon/releases/tag/v0.1.12
