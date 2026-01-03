# Termilyon

Termilyon is a GTK4 + VTE based terminal emulator for Linux. It supports tabs, splits, configurable keybindings, and theme files loaded from TOML.

## Features

- Tabs with close buttons
- Vertical and horizontal splits
- Custom keybindings
- Theme files (TOML)
- Scrollback configuration
- Configurable shell and fonts

## Requirements

System packages (Arch Linux names):

- `gtk4`
- `vte4`

Build tooling:

- Rust toolchain (`cargo`, `rustc`)

## Build & Run

```sh
cargo run
```

## Configuration

Default config path:

`~/.config/termilyon/config.toml`

Sample config:

```toml
scrollback_lines = 10000
font = "Fira Code 12"
font_size = 12
shell = "/bin/bash"
tab_title = "Terminal"
tab_bar_position = "top"
theme_file = "themes/catppuccin-mocha.toml"

[keybindings]
new_tab = "Ctrl+Shift+T"
close_tab = "Ctrl+Shift+W"
rename_tab = "Ctrl+Shift+R"
close_panel = "Ctrl+D"
split_vertical = "Ctrl+Shift+P"
split_horizontal = "Ctrl+Shift+H"
copy = "Ctrl+Shift+C"
paste = "Ctrl+Shift+V"
reload_config = "Ctrl+Shift+L"
show_keybindings = "Ctrl+Shift+K"
focus_left = "Alt+Left"
focus_right = "Alt+Right"
focus_up = "Alt+Up"
focus_down = "Alt+Down"
tab_1 = "Alt+1"
tab_2 = "Alt+2"
tab_3 = "Alt+3"
tab_4 = "Alt+4"
tab_5 = "Alt+5"
tab_6 = "Alt+6"
tab_7 = "Alt+7"
tab_8 = "Alt+8"
tab_9 = "Alt+9"
```

Sample config file:

`examples/config/config.toml`

## Themes

Theme files are TOML and loaded via `theme_file`. The path can be absolute or relative to the config directory.

Example theme files:

- `examples/themes/catppuccin-latte.toml`
- `examples/themes/catppuccin-frappe.toml`
- `examples/themes/catppuccin-macchiato.toml`
- `examples/themes/catppuccin-mocha.toml`
- `examples/themes/darcula.toml`

Theme format:

```toml
background = "#1e1e2e"
foreground = "#cdd6f4"
cursor = "#f5e0dc"
palette = [
  "#45475a",
  "#f38ba8",
  "#a6e3a1",
  "#f9e2af",
  "#89b4fa",
  "#f5c2e7",
  "#94e2d5",
  "#bac2de",
  "#585b70",
  "#f38ba8",
  "#a6e3a1",
  "#f9e2af",
  "#89b4fa",
  "#f5c2e7",
  "#94e2d5",
  "#a6adc8"
]
```

Optional tab colors:

```toml
tab_active_bg = "#1e1e2e"
tab_active_fg = "#cdd6f4"
tab_inactive_bg = "#181825"
tab_inactive_fg = "#a6adc8"
```

## CLI

Override theme file for this run:

```sh
cargo run -- --theme-file /path/to/theme.toml
```

## Keybindings

Defaults (all can be changed via config):

- `Ctrl+Shift+T`: new tab
- `Ctrl+Shift+W`: close tab
- `Ctrl+Shift+R`: rename tab
- `Ctrl+D`: close focused panel (close tab/window if last)
- `Ctrl+Shift+V`: split vertical (left/right)
- `Ctrl+Shift+H`: split horizontal (top/bottom)
- `Ctrl+Shift+L`: reload config/theme
- `Ctrl+Shift+K`: show keybindings
- `Alt+Left/Right/Up/Down`: move focus between splits
- `Alt+1..9`: switch tab

## Split/Exit Behavior

- `Ctrl+D` closes the focused panel. If there is no split, it closes the tab. If it is the last tab, it closes the window.
- Typing `exit` in the shell closes the focused panel/tab/window with the same rules.

## AUR Packaging

Files included:

- `PKGBUILD`
- `.SRCINFO`

Before publishing to AUR:

- Replace `url` with your repo URL in `PKGBUILD` and `.SRCINFO`.
- Replace `sha256sums` with the real checksum (do not use `SKIP`).
- Regenerate `.SRCINFO` after changes:

```sh
makepkg --printsrcinfo > .SRCINFO
```

## License

MIT (update if your project uses a different license).
