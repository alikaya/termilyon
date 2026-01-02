use std::cell::Cell;
use std::env;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use clap::Parser;
use directories::ProjectDirs;
use gtk4 as gtk;
use gtk::gdk;
use gtk::prelude::*;
use serde::Deserialize;
use vte4::prelude::*;
use vte4::{PtyFlags, Terminal};

#[derive(Debug, Clone)]
struct Config {
    scrollback_lines: i32,
    font: String,
    font_size: i32,
    shell: String,
    tab_title: String,
    theme_file: Option<PathBuf>,
    keybindings: KeyBindings,
}

#[derive(Debug, Deserialize)]
struct RawConfig {
    scrollback_lines: Option<i32>,
    font: Option<String>,
    font_size: Option<i32>,
    shell: Option<String>,
    tab_title: Option<String>,
    theme_file: Option<String>,
    keybindings: Option<RawKeyBindings>,
}

#[derive(Debug, Parser)]
#[command(name = "termilyon")]
struct CliArgs {
    #[arg(long)]
    theme_file: Option<PathBuf>,
}

#[derive(Debug, Clone)]
struct KeyBindings {
    new_tab: KeyBinding,
    close_tab: KeyBinding,
    rename_tab: KeyBinding,
    close_panel: KeyBinding,
    split_vertical: KeyBinding,
    split_horizontal: KeyBinding,
    focus_left: KeyBinding,
    focus_right: KeyBinding,
    focus_up: KeyBinding,
    focus_down: KeyBinding,
    tab_switch: Vec<KeyBinding>,
}

#[derive(Debug, Clone, Copy)]
struct KeyBinding {
    key: gdk::Key,
    modifiers: gdk::ModifierType,
}

#[derive(Debug, Deserialize)]
struct RawKeyBindings {
    new_tab: Option<String>,
    close_tab: Option<String>,
    rename_tab: Option<String>,
    close_panel: Option<String>,
    split_vertical: Option<String>,
    split_horizontal: Option<String>,
    focus_left: Option<String>,
    focus_right: Option<String>,
    focus_up: Option<String>,
    focus_down: Option<String>,
    tab_1: Option<String>,
    tab_2: Option<String>,
    tab_3: Option<String>,
    tab_4: Option<String>,
    tab_5: Option<String>,
    tab_6: Option<String>,
    tab_7: Option<String>,
    tab_8: Option<String>,
    tab_9: Option<String>,
}

impl Config {
    fn load() -> Self {
        let default_shell = env::var("SHELL").unwrap_or_else(|_| "/bin/bash".to_string());
        let mut config = Config {
            scrollback_lines: 10_000,
            font: "Fira Code 12".to_string(),
            font_size: 12,
            shell: default_shell,
            tab_title: "Terminal".to_string(),
            theme_file: None,
            keybindings: default_keybindings(),
        };

        if let Some(path) = config_path() {
            if let Ok(contents) = fs::read_to_string(&path) {
                if let Ok(raw) = toml::from_str::<RawConfig>(&contents) {
                    if let Some(lines) = raw.scrollback_lines {
                        config.scrollback_lines = lines;
                    }
                    if let Some(font) = raw.font {
                        config.font = font;
                    }
                    if let Some(size) = raw.font_size {
                        config.font_size = size;
                    }
                    if let Some(shell) = raw.shell {
                        config.shell = shell;
                    }
                    if let Some(tab_title) = raw.tab_title {
                        config.tab_title = tab_title;
                    }
                    if let Some(theme_file) = raw.theme_file {
                        config.theme_file = resolve_theme_path(&path, &theme_file);
                    }
                    if let Some(raw_keys) = raw.keybindings {
                        apply_keybindings(&mut config.keybindings, raw_keys);
                    }
                }
            }
        }

        config
    }
}

fn config_path() -> Option<PathBuf> {
    ProjectDirs::from("io", "termilyon", "termilyon")
        .map(|dirs| dirs.config_dir().join("config.toml"))
}

fn resolve_theme_path(config_path: &PathBuf, theme_file: &str) -> Option<PathBuf> {
    let candidate = PathBuf::from(theme_file);
    if candidate.is_absolute() {
        return Some(candidate);
    }
    let base = config_path.parent().unwrap_or_else(|| std::path::Path::new("."));
    Some(base.join(candidate))
}

fn main() {
    let args = CliArgs::parse();
    let app = gtk::Application::new(
        Some("io.termilyon.app"),
        gtk::gio::ApplicationFlags::FLAGS_NONE,
    );

    app.connect_activate(move |app| build_ui(app, &args));
    app.run();
}

fn build_ui(app: &gtk::Application, args: &CliArgs) {
    let config = Rc::new(Config::load());
    let tab_counter = Rc::new(Cell::new(1));

    let config = if let Some(path) = args.theme_file.as_ref() {
        let mut updated = (*config).clone();
        updated.theme_file = Some(path.clone());
        Rc::new(updated)
    } else {
        config
    };

    let window = gtk::ApplicationWindow::new(app);
    window.set_title(Some("Termilyon"));
    window.set_default_size(1000, 700);

    let notebook = gtk::Notebook::new();
    notebook.set_hexpand(true);
    notebook.set_vexpand(true);
    window.set_child(Some(&notebook));

    create_tab(&notebook, &config, &tab_counter);

    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let notebook_clone = notebook.clone();
    let config_clone = config.clone();
    let counter_clone = tab_counter.clone();
    let window_clone = window.clone();
    controller.connect_key_pressed(move |_, key, _, state| {
        if config_clone.keybindings.new_tab.matches(key, state) {
            create_tab(&notebook_clone, &config_clone, &counter_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.keybindings.close_tab.matches(key, state) {
            close_current_tab(&notebook_clone, &config_clone, &counter_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.keybindings.rename_tab.matches(key, state) {
            rename_current_tab(&window_clone, &notebook_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.keybindings.close_panel.matches(key, state) {
            if close_focused_panel(window_clone.upcast_ref(), &notebook_clone) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone
            .keybindings
            .split_vertical
            .matches(key, state)
        {
            split_current_tab(&notebook_clone, &config_clone, gtk::Orientation::Horizontal);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone
            .keybindings
            .split_horizontal
            .matches(key, state)
        {
            split_current_tab(&notebook_clone, &config_clone, gtk::Orientation::Vertical);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.keybindings.focus_left.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Left) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.keybindings.focus_right.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Right) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.keybindings.focus_up.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Up) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.keybindings.focus_down.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Down) {
                return gtk::glib::Propagation::Stop;
            }
        }

        for (index, binding) in config_clone.keybindings.tab_switch.iter().enumerate() {
            if binding.matches(key, state) {
                let target = index as u32;
                if target < notebook_clone.n_pages() {
                    notebook_clone.set_current_page(Some(target));
                    return gtk::glib::Propagation::Stop;
                }
                break;
            }
        }

        gtk::glib::Propagation::Proceed
    });
    window.add_controller(controller);

    window.present();
}

fn create_tab(notebook: &gtk::Notebook, config: &Rc<Config>, counter: &Rc<Cell<u32>>) {
    let terminal_widget = create_terminal_widget(config);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.append(&terminal_widget.scrolled);

    let tab_index = counter.get();
    counter.set(tab_index + 1);
    let label_text = format!("{} {}", config.tab_title, tab_index);
    let label = gtk::Label::new(Some(&label_text));
    let tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tab_box.append(&label);

    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.set_focusable(false);
    close_button.add_css_class("flat");

    let notebook_clone = notebook.clone();
    let content_clone = content.clone();
    let config_clone = Rc::clone(config);
    let counter_clone = Rc::clone(counter);
    close_button.connect_clicked(move |_| {
        if let Some(page) = notebook_clone.page_num(&content_clone) {
            notebook_clone.remove_page(Some(page));
        }

        if notebook_clone.n_pages() == 0 {
            create_tab(&notebook_clone, &config_clone, &counter_clone);
        }
    });

    tab_box.append(&close_button);

    notebook.append_page(&content, Some(&tab_box));
    notebook.set_current_page(Some(tab_index - 1));
    terminal_widget.terminal.grab_focus();
}

fn close_current_tab(notebook: &gtk::Notebook, config: &Rc<Config>, counter: &Rc<Cell<u32>>) {
    if let Some(page) = notebook.current_page() {
        notebook.remove_page(Some(page));
    }

    if notebook.n_pages() == 0 {
        create_tab(notebook, config, counter);
    }
}

fn rename_current_tab(window: &gtk::ApplicationWindow, notebook: &gtk::Notebook) {
    let Some(page) = notebook.current_page() else { return };
    let Some(child) = notebook.nth_page(Some(page)) else { return };
    let Some(tab_widget) = notebook.tab_label(&child) else { return };
    let Some(label) = find_tab_label(&tab_widget) else { return };

    let dialog = gtk::Dialog::new();
    dialog.set_title(Some("Rename Tab"));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(window));

    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("Rename", gtk::ResponseType::Ok);

    let content = dialog.content_area();
    let entry = gtk::Entry::new();
    entry.set_text(&label.text());
    entry.set_activates_default(true);
    content.append(&entry);

    dialog.set_default_response(gtk::ResponseType::Ok);
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Ok {
            let text = entry.text();
            let new_title = text.trim();
            if !new_title.is_empty() {
                label.set_text(new_title);
            }
        }
        dialog.close();
    });

    dialog.present();
}

fn find_tab_label(tab_widget: &gtk::Widget) -> Option<gtk::Label> {
    let mut child = tab_widget.first_child();
    while let Some(widget) = child {
        if let Ok(label) = widget.clone().downcast::<gtk::Label>() {
            return Some(label);
        }
        child = widget.next_sibling();
    }
    None
}

fn split_current_tab(
    notebook: &gtk::Notebook,
    config: &Rc<Config>,
    orientation: gtk::Orientation,
) {
    let Some(page) = notebook.current_page() else { return };
    let Some(root) = notebook.nth_page(Some(page)) else { return };
    let Ok(root_box) = root.clone().downcast::<gtk::Box>() else { return };

    let existing_child = find_root_window(&root.upcast::<gtk::Widget>())
        .and_then(|window| gtk::prelude::GtkWindowExt::focus(&window))
        .and_then(|focus| find_scrolled_ancestor(&focus))
        .map(|scrolled| scrolled.upcast::<gtk::Widget>())
        .or_else(|| root_box.first_child());
    let Some(existing_child) = existing_child else { return };

    let new_terminal = create_terminal_widget(config);
    let paned = gtk::Paned::new(orientation);
    paned.set_wide_handle(true);
    paned.set_hexpand(true);
    paned.set_vexpand(true);

    replace_widget_in_parent(&existing_child, paned.upcast_ref());

    paned.set_start_child(Some(&existing_child));
    paned.set_end_child(Some(&new_terminal.scrolled));
    new_terminal.terminal.grab_focus();
}

fn close_focused_panel(window: &gtk::Window, notebook: &gtk::Notebook) -> bool {
    let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) else { return false };
    let Some(scrolled) = find_scrolled_ancestor(&focus) else { return false };
    close_scrolled_widget(window, notebook, &scrolled);
    true
}

#[derive(Debug, Clone, Copy)]
enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
}

fn focus_adjacent_split(window: &gtk::Window, direction: FocusDirection) -> bool {
    let Some(focus) = gtk::prelude::GtkWindowExt::focus(window) else { return false };
    let Some(scrolled) = find_scrolled_ancestor(&focus) else { return false };
    let Some(target) = find_adjacent_terminal(&scrolled, direction) else { return false };
    target.grab_focus();
    true
}

fn find_adjacent_terminal(
    scrolled: &gtk::ScrolledWindow,
    direction: FocusDirection,
) -> Option<Terminal> {
    let mut current = scrolled.clone().upcast::<gtk::Widget>();
    let target_orientation = match direction {
        FocusDirection::Left | FocusDirection::Right => gtk::Orientation::Horizontal,
        FocusDirection::Up | FocusDirection::Down => gtk::Orientation::Vertical,
    };

    loop {
        let parent = current.parent()?;
        if let Ok(paned) = parent.clone().downcast::<gtk::Paned>() {
            if paned.orientation() == target_orientation {
                let start = paned.start_child();
                let end = paned.end_child();
                let sibling = match direction {
                    FocusDirection::Left if end.as_ref() == Some(&current) => start,
                    FocusDirection::Right if start.as_ref() == Some(&current) => end,
                    FocusDirection::Up if end.as_ref() == Some(&current) => start,
                    FocusDirection::Down if start.as_ref() == Some(&current) => end,
                    _ => None,
                };
                if let Some(sibling) = sibling {
                    if let Some(terminal) = find_terminal_in_widget(&sibling) {
                        return Some(terminal);
                    }
                }
            }
        }
        current = parent;
    }
}

fn find_terminal_in_widget(widget: &gtk::Widget) -> Option<Terminal> {
    if let Ok(terminal) = widget.clone().downcast::<Terminal>() {
        return Some(terminal);
    }
    if let Ok(scrolled) = widget.clone().downcast::<gtk::ScrolledWindow>() {
        if let Some(child) = scrolled.child() {
            if let Some(terminal) = find_terminal_in_widget(&child) {
                return Some(terminal);
            }
        }
    }
    if let Ok(paned) = widget.clone().downcast::<gtk::Paned>() {
        if let Some(child) = paned.start_child() {
            if let Some(terminal) = find_terminal_in_widget(&child) {
                return Some(terminal);
            }
        }
        if let Some(child) = paned.end_child() {
            if let Some(terminal) = find_terminal_in_widget(&child) {
                return Some(terminal);
            }
        }
    }

    let mut child = widget.first_child();
    while let Some(node) = child {
        if let Some(terminal) = find_terminal_in_widget(&node) {
            return Some(terminal);
        }
        child = node.next_sibling();
    }

    None
}

fn close_scrolled_widget(
    window: &gtk::Window,
    notebook: &gtk::Notebook,
    scrolled: &gtk::ScrolledWindow,
) {
    let Some(parent) = scrolled.parent() else { return };
    if parent.clone().downcast::<gtk::Box>().is_ok() {
        close_tab_or_window(window, notebook);
        return;
    }

    let Ok(paned) = parent.downcast::<gtk::Paned>() else { return };
    collapse_paned(paned, scrolled);
}

fn collapse_paned(paned: gtk::Paned, removed: &gtk::ScrolledWindow) {
    let removed_widget = removed.clone().upcast::<gtk::Widget>();
    let start = paned.start_child();
    let end = paned.end_child();
    let remaining = if start.as_ref() == Some(&removed_widget) {
        end
    } else if end.as_ref() == Some(&removed_widget) {
        start
    } else {
        return;
    };

    paned.set_start_child(None::<&gtk::Widget>);
    paned.set_end_child(None::<&gtk::Widget>);

    let Some(remaining) = remaining else { return };
    replace_widget_in_parent(&paned.upcast::<gtk::Widget>(), &remaining);
}

fn replace_widget_in_parent(old: &gtk::Widget, replacement: &gtk::Widget) {
    let Some(parent) = old.parent() else { return };
    if let Ok(container) = parent.clone().downcast::<gtk::Box>() {
        container.remove(old);
        container.append(replacement);
        return;
    }

    if let Ok(paned) = parent.downcast::<gtk::Paned>() {
        if paned.start_child().as_ref() == Some(old) {
            paned.set_start_child(Some(replacement));
        } else if paned.end_child().as_ref() == Some(old) {
            paned.set_end_child(Some(replacement));
        }
    }
}

fn find_scrolled_ancestor(widget: &gtk::Widget) -> Option<gtk::ScrolledWindow> {
    let mut current = Some(widget.clone());
    while let Some(node) = current {
        if let Ok(scrolled) = node.clone().downcast::<gtk::ScrolledWindow>() {
            return Some(scrolled);
        }
        current = node.parent();
    }
    None
}

fn spawn_shell(terminal: &Terminal, config: &Config) {
    let shell = config.shell.clone();
    let argv = [shell.as_str()];
    let cwd = env::var("HOME").unwrap_or_else(|_| "/".to_string());

    terminal.spawn_async(
        PtyFlags::DEFAULT,
        Some(&cwd),
        &argv,
        &[],
        gtk::glib::SpawnFlags::DEFAULT,
        || {},
        -1,
        None::<&gtk::gio::Cancellable>,
        |result| {
            if let Err(err) = result {
                eprintln!("spawn failed: {err}");
            }
        },
    );
}

struct TerminalWidget {
    scrolled: gtk::ScrolledWindow,
    terminal: Terminal,
}

fn create_terminal_widget(config: &Config) -> TerminalWidget {
    let terminal = Terminal::new();
    terminal.set_scrollback_lines(config.scrollback_lines.into());

    let mut font_desc = gtk::pango::FontDescription::from_string(&config.font);
    if config.font_size > 0 {
        font_desc.set_size(config.font_size * gtk::pango::SCALE);
    }
    terminal.set_font(Some(&font_desc));

    if let Some(theme_path) = config.theme_file.as_ref() {
        if let Some(theme) = theme_from_file(theme_path) {
            apply_theme(&terminal, &theme);
        }
    }

    spawn_shell(&terminal, config);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_child(Some(&terminal));
    scrolled.set_hexpand(true);
    scrolled.set_vexpand(true);

    let scrolled_clone = scrolled.clone();
    terminal.connect_child_exited(move |_, _| {
        close_scrolled_widget_auto(&scrolled_clone);
    });

    TerminalWidget { scrolled, terminal }
}

fn close_scrolled_widget_auto(scrolled: &gtk::ScrolledWindow) {
    let widget = scrolled.clone().upcast::<gtk::Widget>();
    let Some(notebook) = find_parent_notebook(&widget) else { return };
    let Some(window) = find_root_window(&widget) else { return };
    close_scrolled_widget(&window, &notebook, scrolled);
}

fn find_parent_notebook(widget: &gtk::Widget) -> Option<gtk::Notebook> {
    let mut current = widget.parent();
    while let Some(node) = current {
        if let Ok(notebook) = node.clone().downcast::<gtk::Notebook>() {
            return Some(notebook);
        }
        current = node.parent();
    }
    None
}

fn find_root_window(widget: &gtk::Widget) -> Option<gtk::Window> {
    widget
        .root()
        .and_then(|root| root.downcast::<gtk::Window>().ok())
}

#[derive(Debug, Clone)]
struct Theme {
    background: gdk::RGBA,
    foreground: gdk::RGBA,
    cursor: gdk::RGBA,
    palette: [gdk::RGBA; 16],
}

#[derive(Debug, Deserialize)]
struct ThemeConfig {
    background: String,
    foreground: String,
    cursor: String,
    palette: Vec<String>,
}

fn apply_theme(terminal: &Terminal, theme: &Theme) {
    let palette_refs: Vec<&gdk::RGBA> = theme.palette.iter().collect();
    terminal.set_colors(
        Some(&theme.foreground),
        Some(&theme.background),
        &palette_refs,
    );
    terminal.set_color_cursor(Some(&theme.cursor));
}

fn theme_from_file(path: &PathBuf) -> Option<Theme> {
    let Ok(contents) = fs::read_to_string(path) else {
        eprintln!("theme load failed: {}", path.display());
        return None;
    };
    let Ok(raw) = toml::from_str::<ThemeConfig>(&contents) else {
        eprintln!("theme parse failed: {}", path.display());
        return None;
    };

    let palette = parse_palette(&raw.palette)?;
    Some(Theme {
        background: rgba(&raw.background),
        foreground: rgba(&raw.foreground),
        cursor: rgba(&raw.cursor),
        palette,
    })
}

fn rgba(hex: &str) -> gdk::RGBA {
    let hex = hex.trim_start_matches('#');
    let (r, g, b) = match hex.len() {
        6 => {
            let r = u8::from_str_radix(&hex[0..2], 16).unwrap_or(0);
            let g = u8::from_str_radix(&hex[2..4], 16).unwrap_or(0);
            let b = u8::from_str_radix(&hex[4..6], 16).unwrap_or(0);
            (r, g, b)
        }
        _ => (0, 0, 0),
    };
    gdk::RGBA::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
}

fn parse_palette(values: &[String]) -> Option<[gdk::RGBA; 16]> {
    if values.len() != 16 {
        return None;
    }
    let mut colors = values.iter().map(|v| rgba(v)).collect::<Vec<_>>();
    Some([
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
        colors.remove(0),
    ])
}

fn close_tab_or_window(window: &gtk::Window, notebook: &gtk::Notebook) {
    if let Some(page) = notebook.current_page() {
        if notebook.n_pages() <= 1 {
            window.close();
        } else {
            notebook.remove_page(Some(page));
        }
    }
}

fn default_keybindings() -> KeyBindings {
    KeyBindings {
        new_tab: parse_keybinding("Ctrl+Shift+T").unwrap(),
        close_tab: parse_keybinding("Ctrl+Shift+W").unwrap(),
        rename_tab: parse_keybinding("Ctrl+Shift+R").unwrap(),
        close_panel: parse_keybinding("Ctrl+D").unwrap(),
        split_vertical: parse_keybinding("Ctrl+Shift+V").unwrap(),
        split_horizontal: parse_keybinding("Ctrl+Shift+H").unwrap(),
        focus_left: parse_keybinding("Alt+Left").unwrap(),
        focus_right: parse_keybinding("Alt+Right").unwrap(),
        focus_up: parse_keybinding("Alt+Up").unwrap(),
        focus_down: parse_keybinding("Alt+Down").unwrap(),
        tab_switch: (1..=9)
            .map(|n| parse_keybinding(&format!("Alt+{n}")).unwrap())
            .collect(),
    }
}

fn apply_keybindings(bindings: &mut KeyBindings, raw: RawKeyBindings) {
    if let Some(value) = raw.new_tab.and_then(|s| parse_keybinding(&s)) {
        bindings.new_tab = value;
    }
    if let Some(value) = raw.close_tab.and_then(|s| parse_keybinding(&s)) {
        bindings.close_tab = value;
    }
    if let Some(value) = raw.rename_tab.and_then(|s| parse_keybinding(&s)) {
        bindings.rename_tab = value;
    }
    if let Some(value) = raw.close_panel.and_then(|s| parse_keybinding(&s)) {
        bindings.close_panel = value;
    }
    if let Some(value) = raw.split_vertical.and_then(|s| parse_keybinding(&s)) {
        bindings.split_vertical = value;
    }
    if let Some(value) = raw.split_horizontal.and_then(|s| parse_keybinding(&s)) {
        bindings.split_horizontal = value;
    }
    if let Some(value) = raw.focus_left.and_then(|s| parse_keybinding(&s)) {
        bindings.focus_left = value;
    }
    if let Some(value) = raw.focus_right.and_then(|s| parse_keybinding(&s)) {
        bindings.focus_right = value;
    }
    if let Some(value) = raw.focus_up.and_then(|s| parse_keybinding(&s)) {
        bindings.focus_up = value;
    }
    if let Some(value) = raw.focus_down.and_then(|s| parse_keybinding(&s)) {
        bindings.focus_down = value;
    }

    let tabs = [
        raw.tab_1, raw.tab_2, raw.tab_3, raw.tab_4, raw.tab_5, raw.tab_6, raw.tab_7, raw.tab_8,
        raw.tab_9,
    ];
    for (index, entry) in tabs.into_iter().enumerate() {
        if let Some(value) = entry.and_then(|s| parse_keybinding(&s)) {
            if index < bindings.tab_switch.len() {
                bindings.tab_switch[index] = value;
            }
        }
    }
}

fn parse_keybinding(text: &str) -> Option<KeyBinding> {
    let mut modifiers = gdk::ModifierType::empty();
    let mut key: Option<gdk::Key> = None;

    for part in text.split('+') {
        let token = part.trim();
        if token.is_empty() {
            continue;
        }

        match token.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= gdk::ModifierType::CONTROL_MASK,
            "shift" => modifiers |= gdk::ModifierType::SHIFT_MASK,
            "alt" | "option" => modifiers |= gdk::ModifierType::ALT_MASK,
            "super" | "meta" | "win" => modifiers |= gdk::ModifierType::SUPER_MASK,
            _ => {
                let try_name = gdk::Key::from_name(token)
                    .or_else(|| gdk::Key::from_name(&token.to_ascii_uppercase()));
                key = try_name.or(key);
            }
        }
    }

    key.map(|key| KeyBinding { key, modifiers })
}

impl KeyBinding {
    fn matches(&self, key: gdk::Key, state: gdk::ModifierType) -> bool {
        let relevant = gdk::ModifierType::CONTROL_MASK
            | gdk::ModifierType::SHIFT_MASK
            | gdk::ModifierType::ALT_MASK
            | gdk::ModifierType::SUPER_MASK;
        let key_matches = key == self.key
            || key.to_lower() == self.key.to_lower()
            || key.to_upper() == self.key.to_upper();
        key_matches && state & relevant == self.modifiers
    }
}
