use std::cell::{Cell, RefCell};
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
use vte4::{Format, PtyFlags, Terminal};

#[derive(Debug, Clone)]
struct Config {
    scrollback_lines: i32,
    font: String,
    font_size: i32,
    shell: String,
    tab_title: String,
    tab_bar_position: gtk::PositionType,
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
    tab_bar_position: Option<String>,
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
    copy: KeyBinding,
    paste: KeyBinding,
    reload_config: KeyBinding,
    show_keybindings: KeyBinding,
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
    copy: Option<String>,
    paste: Option<String>,
    reload_config: Option<String>,
    show_keybindings: Option<String>,
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
            tab_bar_position: gtk::PositionType::Top,
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
                    if let Some(position) = raw.tab_bar_position {
                        if let Some(parsed) = parse_tab_bar_position(&position) {
                            config.tab_bar_position = parsed;
                        }
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
    let config = Rc::new(RefCell::new(Config::load()));
    let tab_counter = Rc::new(Cell::new(1));

    if let Some(path) = args.theme_file.as_ref() {
        config.borrow_mut().theme_file = Some(path.clone());
    }

    let window = gtk::ApplicationWindow::new(app);
    window.set_title(Some("Termilyon"));
    window.set_default_size(1000, 700);
    window.set_decorated(false);

    let notebook = gtk::Notebook::new();
    notebook.set_hexpand(true);
    notebook.set_vexpand(true);
    notebook.add_css_class("terminal-tabs");
    notebook.set_tab_pos(config.borrow().tab_bar_position);
    notebook.connect_switch_page(|notebook, _, page| {
        focus_terminal_in_page(notebook, page);
    });
    window.set_child(Some(&notebook));

    let theme = config
        .borrow()
        .theme_file
        .as_ref()
        .and_then(|path| theme_from_file(path));
    let first_terminal = create_tab(&notebook, &config, &tab_counter);
    apply_tab_styles(&notebook, theme.as_ref(), Some(&first_terminal));

    let controller = gtk::EventControllerKey::new();
    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    let notebook_clone = notebook.clone();
    let config_clone = config.clone();
    let counter_clone = tab_counter.clone();
    let window_clone = window.clone();
    let theme_override = args.theme_file.clone();
    controller.connect_key_pressed(move |_, key, _, state| {
        if config_clone.borrow().keybindings.new_tab.matches(key, state) {
            create_tab(&notebook_clone, &config_clone, &counter_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.borrow().keybindings.close_tab.matches(key, state) {
            close_current_tab(&notebook_clone, &config_clone, &counter_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.borrow().keybindings.rename_tab.matches(key, state) {
            rename_current_tab(&window_clone, &notebook_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.borrow().keybindings.close_panel.matches(key, state) {
            if close_focused_panel(window_clone.upcast_ref(), &notebook_clone) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone
            .borrow()
            .keybindings
            .split_vertical
            .matches(key, state)
        {
            split_current_tab(&notebook_clone, &config_clone, gtk::Orientation::Horizontal);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone
            .borrow()
            .keybindings
            .split_horizontal
            .matches(key, state)
        {
            split_current_tab(&notebook_clone, &config_clone, gtk::Orientation::Vertical);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone.borrow().keybindings.copy.matches(key, state) {
            if let Some(terminal) = focused_terminal(window_clone.upcast_ref()) {
                terminal.copy_clipboard_format(Format::Text);
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.borrow().keybindings.paste.matches(key, state) {
            if let Some(terminal) = focused_terminal(window_clone.upcast_ref()) {
                terminal.paste_clipboard();
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.borrow().keybindings.focus_left.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Left) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.borrow().keybindings.focus_right.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Right) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.borrow().keybindings.focus_up.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Up) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone.borrow().keybindings.focus_down.matches(key, state) {
            if focus_adjacent_split(window_clone.upcast_ref(), FocusDirection::Down) {
                return gtk::glib::Propagation::Stop;
            }
        }

        if config_clone
            .borrow()
            .keybindings
            .reload_config
            .matches(key, state)
        {
            reload_config_and_theme(
                &config_clone,
                &notebook_clone,
                theme_override.as_ref(),
            );
            return gtk::glib::Propagation::Stop;
        }

        if config_clone
            .borrow()
            .keybindings
            .show_keybindings
            .matches(key, state)
        {
            let config_ref = config_clone.borrow();
            show_keybindings_dialog(&window_clone, &config_ref);
            return gtk::glib::Propagation::Stop;
        }

        for (index, binding) in config_clone
            .borrow()
            .keybindings
            .tab_switch
            .iter()
            .enumerate()
        {
            if binding.matches(key, state) {
                let target = index as u32;
                if target < notebook_clone.n_pages() {
                    notebook_clone.set_current_page(Some(target));
                    focus_terminal_in_page(&notebook_clone, target);
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

fn create_tab(
    notebook: &gtk::Notebook,
    config: &Rc<RefCell<Config>>,
    counter: &Rc<Cell<u32>>,
) -> Terminal {
    let config_ref = config.borrow();
    let terminal_widget = create_terminal_widget(&config_ref);
    let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
    content.set_hexpand(true);
    content.set_vexpand(true);
    content.append(&terminal_widget.scrolled);

    let tab_index = counter.get();
    counter.set(tab_index + 1);
    let label_text = format!("{} {}", config_ref.tab_title, tab_index);
    let label = gtk::Label::new(Some(&label_text));
    label.add_css_class("terminal-tab-label");
    let tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 6);
    tab_box.add_css_class("terminal-tab");
    tab_box.append(&label);

    let close_button = gtk::Button::from_icon_name("window-close-symbolic");
    close_button.set_focusable(false);
    close_button.add_css_class("flat");

    let notebook_clone = notebook.clone();
    let content_clone = content.clone();
    let config_clone = Rc::clone(config);
    let counter_clone = Rc::clone(counter);
    close_button.connect_clicked(move |_| {
        let page = notebook_clone.page_num(&content_clone);
        if let Some(page) = page {
            notebook_clone.remove_page(Some(page));
            focus_previous_tab(&notebook_clone, page);
        }

        if notebook_clone.n_pages() == 0 {
            create_tab(&notebook_clone, &config_clone, &counter_clone);
        }
    });

    tab_box.append(&close_button);

    notebook.append_page(&content, Some(&tab_box));
    notebook.set_current_page(Some(tab_index - 1));
    terminal_widget.terminal.grab_focus();

    terminal_widget.terminal.clone()
}

fn close_current_tab(
    notebook: &gtk::Notebook,
    config: &Rc<RefCell<Config>>,
    counter: &Rc<Cell<u32>>,
) {
    if let Some(page) = notebook.current_page() {
        notebook.remove_page(Some(page));
        focus_previous_tab(notebook, page);
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
    config: &Rc<RefCell<Config>>,
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

    let config_ref = config.borrow();
    let new_terminal = create_terminal_widget(&config_ref);
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

fn focused_terminal(window: &gtk::Window) -> Option<Terminal> {
    let focus = gtk::prelude::GtkWindowExt::focus(window)?;
    find_terminal_in_widget(&focus).or_else(|| {
        find_scrolled_ancestor(&focus)
            .and_then(|scrolled| find_terminal_in_widget(scrolled.upcast_ref()))
    })
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
    tab_active_bg: Option<gdk::RGBA>,
    tab_active_fg: Option<gdk::RGBA>,
    tab_inactive_bg: Option<gdk::RGBA>,
    tab_inactive_fg: Option<gdk::RGBA>,
}

#[derive(Debug, Deserialize)]
struct ThemeConfig {
    background: String,
    foreground: String,
    cursor: String,
    palette: Vec<String>,
    tab_active_bg: Option<String>,
    tab_active_fg: Option<String>,
    tab_inactive_bg: Option<String>,
    tab_inactive_fg: Option<String>,
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
        tab_active_bg: raw.tab_active_bg.as_deref().map(rgba),
        tab_active_fg: raw.tab_active_fg.as_deref().map(rgba),
        tab_inactive_bg: raw.tab_inactive_bg.as_deref().map(rgba),
        tab_inactive_fg: raw.tab_inactive_fg.as_deref().map(rgba),
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

fn apply_tab_styles(
    notebook: &gtk::Notebook,
    theme: Option<&Theme>,
    terminal: Option<&Terminal>,
) {
    let background = terminal
        .map(|terminal| terminal.color_background_for_draw())
        .or_else(|| theme.map(|theme| theme.background.clone()));
    let Some(background) = background else { return };

    let base_fg = theme
        .map(|theme| theme.foreground.clone())
        .unwrap_or_else(|| contrast_text_color(&background));

    let active_bg = theme
        .and_then(|theme| theme.tab_active_bg.clone())
        .unwrap_or_else(|| background.clone());
    let inactive_bg = theme
        .and_then(|theme| theme.tab_inactive_bg.clone())
        .unwrap_or_else(|| adjust_luma(&background, 0.92));

    let active_fg = theme
        .and_then(|theme| theme.tab_active_fg.clone())
        .unwrap_or_else(|| base_fg.clone());
    let inactive_fg = theme
        .and_then(|theme| theme.tab_inactive_fg.clone())
        .unwrap_or_else(|| with_alpha(&base_fg, 0.7));

    let mut css = format!(
        ".terminal-tabs > header {{ background-color: {}; }}",
        background.to_str()
    );
    css.push_str(&format!(
        ".terminal-tabs tab {{ background-color: {}; background-image: none; border-image: none; }}",
        inactive_bg.to_str()
    ));
    css.push_str(&format!(
        ".terminal-tabs tab:checked {{ background-color: {}; background-image: none; border-image: none; }}",
        active_bg.to_str()
    ));
    css.push_str(".terminal-tabs tab > * { background-color: transparent; }");
    css.push_str(&format!(
        ".terminal-tabs tab label, .terminal-tabs tab button {{ color: {}; }}",
        inactive_fg.to_str()
    ));
    css.push_str(&format!(
        ".terminal-tabs tab:checked label, .terminal-tabs tab:checked button {{ color: {}; }}",
        active_fg.to_str()
    ));

    let provider = gtk::CssProvider::new();
    provider.load_from_data(&css);
    if let Some(display) = gdk::Display::default() {
        gtk::style_context_add_provider_for_display(
            &display,
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    notebook.queue_draw();
}

fn reload_config_and_theme(
    config: &Rc<RefCell<Config>>,
    notebook: &gtk::Notebook,
    theme_override: Option<&PathBuf>,
) {
    let mut updated = Config::load();
    if let Some(path) = theme_override {
        updated.theme_file = Some(path.clone());
    }
    let theme = updated
        .theme_file
        .as_ref()
        .and_then(|path| theme_from_file(path));
    *config.borrow_mut() = updated.clone();

    notebook.set_tab_pos(updated.tab_bar_position);
    let sample_terminal = find_first_terminal_in_notebook(notebook);
    apply_tab_styles(notebook, theme.as_ref(), sample_terminal.as_ref());
    apply_config_to_terminals(notebook, &updated, theme.as_ref());
}

fn find_first_terminal_in_notebook(notebook: &gtk::Notebook) -> Option<Terminal> {
    for index in 0..notebook.n_pages() {
        let Some(page) = notebook.nth_page(Some(index)) else { continue };
        if let Some(terminal) = find_terminal_in_widget(&page) {
            return Some(terminal);
        }
    }
    None
}

fn apply_config_to_terminals(
    notebook: &gtk::Notebook,
    config: &Config,
    theme: Option<&Theme>,
) {
    for index in 0..notebook.n_pages() {
        let Some(page) = notebook.nth_page(Some(index)) else { continue };
        let mut terminals = Vec::new();
        collect_terminals(&page, &mut terminals);
        for terminal in terminals {
            terminal.set_scrollback_lines(config.scrollback_lines.into());
            let mut font_desc = gtk::pango::FontDescription::from_string(&config.font);
            if config.font_size > 0 {
                font_desc.set_size(config.font_size * gtk::pango::SCALE);
            }
            terminal.set_font(Some(&font_desc));
            if let Some(theme) = theme {
                apply_theme(&terminal, theme);
            }
        }
    }
}

fn collect_terminals(widget: &gtk::Widget, terminals: &mut Vec<Terminal>) {
    if let Ok(terminal) = widget.clone().downcast::<Terminal>() {
        terminals.push(terminal);
        return;
    }
    if let Ok(scrolled) = widget.clone().downcast::<gtk::ScrolledWindow>() {
        if let Some(child) = scrolled.child() {
            collect_terminals(&child, terminals);
        }
        return;
    }
    if let Ok(paned) = widget.clone().downcast::<gtk::Paned>() {
        if let Some(child) = paned.start_child() {
            collect_terminals(&child, terminals);
        }
        if let Some(child) = paned.end_child() {
            collect_terminals(&child, terminals);
        }
        return;
    }

    let mut child = widget.first_child();
    while let Some(node) = child {
        collect_terminals(&node, terminals);
        child = node.next_sibling();
    }
}

fn show_keybindings_dialog(window: &gtk::ApplicationWindow, config: &Config) {
    let dialog = gtk::Dialog::new();
    dialog.set_title(Some("Keybindings"));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(window));
    dialog.add_button("Close", gtk::ResponseType::Close);

    let content = dialog.content_area();
    let list = gtk::Box::new(gtk::Orientation::Vertical, 6);
    list.set_margin_top(12);
    list.set_margin_bottom(12);
    list.set_margin_start(12);
    list.set_margin_end(12);

    add_keybinding_row(&list, "New tab", &config.keybindings.new_tab);
    add_keybinding_row(&list, "Close tab", &config.keybindings.close_tab);
    add_keybinding_row(&list, "Rename tab", &config.keybindings.rename_tab);
    add_keybinding_row(&list, "Close panel", &config.keybindings.close_panel);
    add_keybinding_row(&list, "Split vertical", &config.keybindings.split_vertical);
    add_keybinding_row(&list, "Split horizontal", &config.keybindings.split_horizontal);
    add_keybinding_row(&list, "Copy", &config.keybindings.copy);
    add_keybinding_row(&list, "Paste", &config.keybindings.paste);
    add_keybinding_row(&list, "Reload config/theme", &config.keybindings.reload_config);
    add_keybinding_row(&list, "Show keybindings", &config.keybindings.show_keybindings);
    add_keybinding_row(&list, "Focus left", &config.keybindings.focus_left);
    add_keybinding_row(&list, "Focus right", &config.keybindings.focus_right);
    add_keybinding_row(&list, "Focus up", &config.keybindings.focus_up);
    add_keybinding_row(&list, "Focus down", &config.keybindings.focus_down);

    for (index, binding) in config.keybindings.tab_switch.iter().enumerate() {
        let title = format!("Switch tab {}", index + 1);
        add_keybinding_row(&list, &title, binding);
    }

    content.append(&list);

    dialog.connect_response(|dialog, _| {
        dialog.close();
    });
    dialog.present();
}

fn add_keybinding_row(container: &gtk::Box, name: &str, binding: &KeyBinding) {
    let row = gtk::Box::new(gtk::Orientation::Horizontal, 12);
    let label = gtk::Label::new(Some(name));
    label.set_xalign(0.0);
    label.set_hexpand(true);
    let shortcut = gtk::Label::new(Some(&format_keybinding(binding)));
    shortcut.set_xalign(1.0);
    row.append(&label);
    row.append(&shortcut);
    container.append(&row);
}

fn format_keybinding(binding: &KeyBinding) -> String {
    let mut parts = Vec::new();
    let modifiers = binding.modifiers;
    if modifiers.contains(gdk::ModifierType::CONTROL_MASK) {
        parts.push("Ctrl".to_string());
    }
    if modifiers.contains(gdk::ModifierType::SHIFT_MASK) {
        parts.push("Shift".to_string());
    }
    if modifiers.contains(gdk::ModifierType::ALT_MASK) {
        parts.push("Alt".to_string());
    }
    if modifiers.contains(gdk::ModifierType::SUPER_MASK) {
        parts.push("Super".to_string());
    }

    let key_name = binding
        .key
        .name()
        .map(|name| name.to_string())
        .unwrap_or_else(|| format!("{:?}", binding.key));
    let key_display = if key_name.len() == 1 {
        key_name.to_ascii_uppercase()
    } else {
        key_name
    };
    parts.push(key_display);
    parts.join("+")
}

fn clamp01(value: f32) -> f32 {
    value.max(0.0).min(1.0)
}

fn adjust_luma(color: &gdk::RGBA, factor: f32) -> gdk::RGBA {
    gdk::RGBA::new(
        clamp01(color.red() * factor),
        clamp01(color.green() * factor),
        clamp01(color.blue() * factor),
        color.alpha(),
    )
}

fn with_alpha(color: &gdk::RGBA, alpha: f32) -> gdk::RGBA {
    gdk::RGBA::new(color.red(), color.green(), color.blue(), clamp01(alpha))
}

fn contrast_text_color(background: &gdk::RGBA) -> gdk::RGBA {
    let luminance = 0.2126 * background.red()
        + 0.7152 * background.green()
        + 0.0722 * background.blue();
    if luminance < 0.5 {
        gdk::RGBA::new(1.0, 1.0, 1.0, 1.0)
    } else {
        gdk::RGBA::new(0.0, 0.0, 0.0, 1.0)
    }
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
            focus_previous_tab(notebook, page);
        }
    }
}

fn default_keybindings() -> KeyBindings {
    KeyBindings {
        new_tab: parse_keybinding("Ctrl+Shift+T").unwrap(),
        close_tab: parse_keybinding("Ctrl+Shift+W").unwrap(),
        rename_tab: parse_keybinding("Ctrl+Shift+R").unwrap(),
        close_panel: parse_keybinding("Ctrl+D").unwrap(),
        split_vertical: parse_keybinding("Ctrl+Shift+P").unwrap(),
        split_horizontal: parse_keybinding("Ctrl+Shift+H").unwrap(),
        copy: parse_keybinding("Ctrl+Shift+C").unwrap(),
        paste: parse_keybinding("Ctrl+Shift+V").unwrap(),
        reload_config: parse_keybinding("Ctrl+Shift+L").unwrap(),
        show_keybindings: parse_keybinding("Ctrl+Shift+K").unwrap(),
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
    if let Some(value) = raw.copy.and_then(|s| parse_keybinding(&s)) {
        bindings.copy = value;
    }
    if let Some(value) = raw.paste.and_then(|s| parse_keybinding(&s)) {
        bindings.paste = value;
    }
    if let Some(value) = raw.reload_config.and_then(|s| parse_keybinding(&s)) {
        bindings.reload_config = value;
    }
    if let Some(value) = raw.show_keybindings.and_then(|s| parse_keybinding(&s)) {
        bindings.show_keybindings = value;
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

fn parse_tab_bar_position(value: &str) -> Option<gtk::PositionType> {
    match value.trim().to_ascii_lowercase().as_str() {
        "top" => Some(gtk::PositionType::Top),
        "bottom" => Some(gtk::PositionType::Bottom),
        _ => None,
    }
}

fn focus_previous_tab(notebook: &gtk::Notebook, closed_index: u32) {
    let pages = notebook.n_pages();
    if pages == 0 {
        return;
    }
    let mut target = if closed_index > 0 { closed_index - 1 } else { 0 };
    if target >= pages {
        target = pages - 1;
    }
    notebook.set_current_page(Some(target));
    focus_terminal_in_page(notebook, target);
}

fn focus_terminal_in_page(notebook: &gtk::Notebook, page: u32) {
    if let Some(child) = notebook.nth_page(Some(page)) {
        if let Some(terminal) = find_terminal_in_widget(&child) {
            terminal.grab_focus();
        }
    }
}
