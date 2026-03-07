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
use serde::{Deserialize, Serialize};
use vte4::prelude::*;
use vte4::{Format, PtyFlags, Terminal};

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SshServer {
    name: String,
    host: String,
    user: String,
    port: u16,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct SshServers {
    servers: Vec<SshServer>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Password {
    name: String,
    password: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
struct Passwords {
    passwords: Vec<Password>,
}

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
    secret: String,
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
    secret: Option<String>,
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
    ssh_manager: KeyBinding,
    password_manager: KeyBinding,
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
    ssh_manager: Option<String>,
    password_manager: Option<String>,
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
            secret: String::new(),
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
                    if let Some(secret) = raw.secret {
                        config.secret = secret;
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
            if let Some(terminal) = focused_terminal(window_clone.upcast_ref()) {
                terminal.feed_child(b"\x04");
            }
            return gtk::glib::Propagation::Stop;
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

        if config_clone
            .borrow()
            .keybindings
            .ssh_manager
            .matches(key, state)
        {
            show_ssh_manager_dialog(&window_clone, &notebook_clone);
            return gtk::glib::Propagation::Stop;
        }

        if config_clone
            .borrow()
            .keybindings
            .password_manager
            .matches(key, state)
        {
            let secret = config_clone.borrow().secret.clone();
            show_password_manager_dialog(&window_clone, &notebook_clone, secret);
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
    attach_font_scroll_handler(&terminal_widget.terminal, config);

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
    attach_font_scroll_handler(&new_terminal.terminal, config);
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

fn attach_font_scroll_handler(terminal: &Terminal, config: &Rc<RefCell<Config>>) {
    let ctrl = gtk::EventControllerScroll::new(gtk::EventControllerScrollFlags::VERTICAL);
    ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
    let terminal = terminal.clone();
    let terminal_ac = terminal.clone();
    let config = config.clone();
    ctrl.connect_scroll(move |ctrl, _dx, dy| {
        let modifiers = ctrl
            .current_event()
            .map(|e| e.modifier_state())
            .unwrap_or(gdk::ModifierType::empty());
        if modifiers.contains(gdk::ModifierType::CONTROL_MASK)
            && modifiers.contains(gdk::ModifierType::SHIFT_MASK)
        {
            let new_size = {
                let mut cfg = config.borrow_mut();
                cfg.font_size = if dy < 0.0 {
                    (cfg.font_size + 1).min(72)
                } else {
                    (cfg.font_size - 1).max(6)
                };
                cfg.font_size
            };
            let cfg = config.borrow();
            let mut font_desc = gtk::pango::FontDescription::from_string(&cfg.font);
            font_desc.set_size(new_size * gtk::pango::SCALE);
            terminal.set_font(Some(&font_desc));
            return gtk::glib::Propagation::Stop;
        }
        gtk::glib::Propagation::Proceed
    });
    terminal_ac.add_controller(ctrl);
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
    add_keybinding_row(&list, "SSH manager", &config.keybindings.ssh_manager);
    add_keybinding_row(&list, "Password manager", &config.keybindings.password_manager);
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

fn derive_key(secret: &str) -> [u8; 32] {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(secret.as_bytes());
    hasher.finalize().into()
}

fn encrypt_password(plaintext: &str, secret: &str) -> Result<String, String> {
    use aes_gcm::aead::{Aead, KeyInit, OsRng};
    use aes_gcm::aead::rand_core::RngCore;
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use base64::{engine::general_purpose::STANDARD, Engine};

    let key_bytes = derive_key(secret);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);

    let mut nonce_bytes = [0u8; 12];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = Nonce::from_slice(&nonce_bytes);

    let ciphertext = cipher
        .encrypt(nonce, plaintext.as_bytes())
        .map_err(|e| e.to_string())?;

    let mut combined = nonce_bytes.to_vec();
    combined.extend_from_slice(&ciphertext);
    Ok(STANDARD.encode(&combined))
}

fn decrypt_password(encrypted: &str, secret: &str) -> Result<String, String> {
    use aes_gcm::aead::{Aead, KeyInit};
    use aes_gcm::{Aes256Gcm, Key, Nonce};
    use base64::{engine::general_purpose::STANDARD, Engine};

    let data = STANDARD.decode(encrypted).map_err(|e| e.to_string())?;
    if data.len() < 13 {
        return Err("Geçersiz şifreli veri".to_string());
    }
    let (nonce_bytes, ciphertext) = data.split_at(12);
    let key_bytes = derive_key(secret);
    let key = Key::<Aes256Gcm>::from_slice(&key_bytes);
    let cipher = Aes256Gcm::new(key);
    let nonce = Nonce::from_slice(nonce_bytes);

    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "Şifre çözme başarısız — secret anahtarı yanlış olabilir".to_string())?;
    String::from_utf8(plaintext).map_err(|e| e.to_string())
}

fn first_visible_index(list_box: &gtk::ListBox) -> Option<i32> {
    let mut idx = 0;
    loop {
        match list_box.row_at_index(idx) {
            None => return None,
            Some(row) if row.is_visible() => return Some(idx),
            _ => idx += 1,
        }
    }
}

fn select_first_visible(list_box: &gtk::ListBox) {
    let mut idx = 0;
    loop {
        match list_box.row_at_index(idx) {
            None => {
                list_box.select_row(None::<&gtk::ListBoxRow>);
                break;
            }
            Some(row) if row.is_visible() => {
                list_box.select_row(Some(&row));
                break;
            }
            _ => idx += 1,
        }
    }
}

fn passwords_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "termilyon")
        .map(|dirs| dirs.config_dir().join("passwords.toml"))
}

fn load_passwords(secret: &str) -> Vec<Password> {
    let Some(path) = passwords_path() else { return Vec::new() };
    let Ok(content) = fs::read_to_string(&path) else { return Vec::new() };
    let encrypted_list = toml::from_str::<Passwords>(&content)
        .map(|p| p.passwords)
        .unwrap_or_default();
    encrypted_list
        .into_iter()
        .filter_map(|p| {
            decrypt_password(&p.password, secret)
                .ok()
                .map(|plaintext| Password { name: p.name, password: plaintext })
        })
        .collect()
}

fn save_passwords(passwords: &[Password], secret: &str) {
    let Some(path) = passwords_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let encrypted: Vec<Password> = passwords
        .iter()
        .filter_map(|p| {
            encrypt_password(&p.password, secret)
                .ok()
                .map(|enc| Password { name: p.name.clone(), password: enc })
        })
        .collect();
    let data = Passwords { passwords: encrypted };
    if let Ok(content) = toml::to_string(&data) {
        let _ = fs::write(&path, content);
    }
}

fn populate_password_list(list_box: &gtk::ListBox, passwords: &[Password]) {
    while let Some(row) = list_box.row_at_index(0) {
        list_box.remove(&row);
    }
    for pwd in passwords {
        let label = gtk::Label::new(Some(&pwd.name));
        label.set_xalign(0.0);
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(8);
        label.set_margin_end(8);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&label));
        list_box.append(&row);
    }
    if let Some(first) = list_box.row_at_index(0) {
        list_box.select_row(Some(&first));
    }
}

fn password_paste_selected(
    dialog: &gtk::Dialog,
    notebook: &gtk::Notebook,
    list_box: &gtk::ListBox,
    passwords: &Rc<RefCell<Vec<Password>>>,
) {
    let Some(row) = list_box.selected_row() else { return };
    let index = row.index() as usize;
    let passwords_ref = passwords.borrow();
    let Some(pwd) = passwords_ref.get(index) else { return };
    let text = format!("{}\n", pwd.password);
    let Some(page) = notebook.current_page() else { return };
    let Some(child) = notebook.nth_page(Some(page)) else { return };
    if let Some(terminal) = find_terminal_in_widget(&child) {
        terminal.feed_child(text.as_bytes());
    }
    dialog.close();
}

fn show_add_password_dialog(
    parent: &gtk::Dialog,
    list_box: &gtk::ListBox,
    passwords: &Rc<RefCell<Vec<Password>>>,
    secret: Rc<String>,
) {
    let dialog = gtk::Dialog::new();
    dialog.set_title(Some("Add Password"));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("Add", gtk::ResponseType::Ok);
    dialog.set_default_response(gtk::ResponseType::Ok);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let grid = gtk::Grid::new();
    grid.set_row_spacing(8);
    grid.set_column_spacing(8);

    let name_label = gtk::Label::new(Some("Name:"));
    name_label.set_xalign(1.0);
    let name_entry = gtk::Entry::new();
    name_entry.set_placeholder_text(Some("sudo, API key, ..."));
    name_entry.set_activates_default(true);

    let pass_label = gtk::Label::new(Some("Password:"));
    pass_label.set_xalign(1.0);
    let pass_entry = gtk::Entry::new();
    pass_entry.set_visibility(false);
    pass_entry.set_input_purpose(gtk::InputPurpose::Password);
    pass_entry.set_activates_default(true);

    grid.attach(&name_label, 0, 0, 1, 1);
    grid.attach(&name_entry, 1, 0, 1, 1);
    grid.attach(&pass_label, 0, 1, 1, 1);
    grid.attach(&pass_entry, 1, 1, 1, 1);
    content.append(&grid);

    let list_box = list_box.clone();
    let passwords = passwords.clone();
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Ok {
            let name = name_entry.text().trim().to_string();
            let password = pass_entry.text().to_string();
            if !name.is_empty() && !password.is_empty() {
                let entry = Password { name, password };
                {
                    let mut pwds = passwords.borrow_mut();
                    pwds.push(entry);
                    save_passwords(&pwds, &secret);
                }
                populate_password_list(&list_box, &passwords.borrow());
            }
        }
        dialog.close();
    });

    dialog.present();
}

fn show_password_manager_dialog(window: &gtk::ApplicationWindow, notebook: &gtk::Notebook, secret: String) {
    if secret.is_empty() {
        let dialog = gtk::Dialog::new();
        dialog.set_title(Some("Şifre Yöneticisi"));
        dialog.set_modal(true);
        dialog.set_transient_for(Some(window));
        dialog.add_button("Tamam", gtk::ResponseType::Ok);
        let content = dialog.content_area();
        content.set_margin_top(16);
        content.set_margin_bottom(16);
        content.set_margin_start(16);
        content.set_margin_end(16);
        let label = gtk::Label::new(Some(
            "Şifre yöneticisini kullanmak için config.toml dosyasına\n\
             secret = \"gizli-anahtarınız\" satırını ekleyin.",
        ));
        label.set_wrap(true);
        content.append(&label);
        dialog.connect_response(|d, _| d.close());
        dialog.present();
        return;
    }

    let secret = Rc::new(secret);
    let passwords = Rc::new(RefCell::new(load_passwords(&secret)));

    let dialog = gtk::Dialog::new();
    dialog.set_title(Some("Passwords"));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(window));
    dialog.set_default_size(400, 340);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Ara..."));
    content.append(&search_entry);

    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.set_vexpand(true);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_child(Some(&list_box));
    scrolled.set_vexpand(true);
    scrolled.set_min_content_height(200);
    content.append(&scrolled);

    populate_password_list(&list_box, &passwords.borrow());

    // Filtre fonksiyonu
    {
        let search = search_entry.clone();
        list_box.set_filter_func(move |row| {
            let text = search.text().to_lowercase();
            if text.is_empty() { return true; }
            row.child()
                .and_then(|c| c.downcast::<gtk::Label>().ok())
                .is_some_and(|l| l.text().to_lowercase().contains(&text))
        });
    }

    // Arama değiştiğinde filtrele ve ilk görünür satırı seç
    {
        let list_box = list_box.clone();
        search_entry.connect_search_changed(move |_| {
            list_box.invalidate_filter();
            select_first_visible(&list_box);
        });
    }

    // Arama kutusunda ↓ → listeye geç; Escape → kapat
    {
        let ctrl = gtk::EventControllerKey::new();
        let list_box = list_box.clone();
        let dialog = dialog.clone();
        ctrl.connect_key_pressed(move |_, key, _, _| match key {
            gdk::Key::Down => {
                if let Some(row) = list_box.selected_row() {
                    row.grab_focus();
                } else {
                    select_first_visible(&list_box);
                    if let Some(row) = list_box.selected_row() {
                        row.grab_focus();
                    }
                }
                gtk::glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                dialog.close();
                gtk::glib::Propagation::Stop
            }
            _ => gtk::glib::Propagation::Proceed,
        });
        search_entry.add_controller(ctrl);
    }

    // Listede ↑ + ilk satır → arama kutusuna dön
    {
        let ctrl = gtk::EventControllerKey::new();
        ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let search = search_entry.clone();
        let list_box_mv = list_box.clone();
        let list_box_ac = list_box.clone();
        ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Up {
                if let Some(row) = list_box_mv.selected_row() {
                    if Some(row.index()) == first_visible_index(&list_box_mv) {
                        search.grab_focus();
                        return gtk::glib::Propagation::Stop;
                    }
                }
            }
            gtk::glib::Propagation::Proceed
        });
        list_box_ac.add_controller(ctrl);
    }

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_margin_top(4);
    btn_box.set_halign(gtk::Align::End);

    let add_btn = gtk::Button::with_label("Add");
    let delete_btn = gtk::Button::with_label("Delete");
    let paste_btn = gtk::Button::with_label("Paste");
    let close_btn = gtk::Button::with_label("Close");
    btn_box.append(&add_btn);
    btn_box.append(&delete_btn);
    btn_box.append(&paste_btn);
    btn_box.append(&close_btn);
    content.append(&btn_box);

    // Add button
    {
        let dialog = dialog.clone();
        let list_box = list_box.clone();
        let passwords = passwords.clone();
        let secret = secret.clone();
        add_btn.connect_clicked(move |_| {
            show_add_password_dialog(&dialog, &list_box, &passwords, secret.clone());
        });
    }

    // Delete button
    {
        let list_box = list_box.clone();
        let passwords = passwords.clone();
        let secret = secret.clone();
        delete_btn.connect_clicked(move |_| {
            let Some(row) = list_box.selected_row() else { return };
            let index = row.index() as usize;
            {
                let mut pwds = passwords.borrow_mut();
                if index < pwds.len() {
                    pwds.remove(index);
                }
                save_passwords(&pwds, &secret);
            }
            populate_password_list(&list_box, &passwords.borrow());
            list_box.invalidate_filter();
            select_first_visible(&list_box);
        });
    }

    // Paste button
    {
        let dialog = dialog.clone();
        let notebook = notebook.clone();
        let passwords = passwords.clone();
        let list_box = list_box.clone();
        paste_btn.connect_clicked(move |_| {
            password_paste_selected(&dialog, &notebook, &list_box, &passwords);
        });
    }

    // Close button
    {
        let dialog = dialog.clone();
        close_btn.connect_clicked(move |_| dialog.close());
    }

    // Row activated: Enter or double-click → paste
    {
        let dialog = dialog.clone();
        let notebook = notebook.clone();
        let passwords = passwords.clone();
        list_box.connect_row_activated(move |lb, _| {
            password_paste_selected(&dialog, &notebook, lb, &passwords);
        });
    }

    // Delete key removes selected entry
    {
        let key_ctrl = gtk::EventControllerKey::new();
        let passwords = passwords.clone();
        let list_box = list_box.clone();
        let secret = secret.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Delete {
                let Some(row) = list_box.selected_row() else {
                    return gtk::glib::Propagation::Proceed;
                };
                let index = row.index() as usize;
                {
                    let mut pwds = passwords.borrow_mut();
                    if index < pwds.len() {
                        pwds.remove(index);
                    }
                    save_passwords(&pwds, &secret);
                }
                populate_password_list(&list_box, &passwords.borrow());
                list_box.invalidate_filter();
                select_first_visible(&list_box);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);
    }

    search_entry.grab_focus();
    dialog.present();
}

fn ssh_servers_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "termilyon")
        .map(|dirs| dirs.config_dir().join("ssh_servers.toml"))
}

fn load_ssh_servers() -> Vec<SshServer> {
    let Some(path) = ssh_servers_path() else { return Vec::new() };
    let Ok(content) = fs::read_to_string(&path) else { return Vec::new() };
    toml::from_str::<SshServers>(&content)
        .map(|s| s.servers)
        .unwrap_or_default()
}

fn save_ssh_servers(servers: &[SshServer]) {
    let Some(path) = ssh_servers_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let data = SshServers { servers: servers.to_vec() };
    if let Ok(content) = toml::to_string(&data) {
        let _ = fs::write(&path, content);
    }
}

fn populate_server_list(list_box: &gtk::ListBox, servers: &[SshServer]) {
    while let Some(row) = list_box.row_at_index(0) {
        list_box.remove(&row);
    }
    for srv in servers {
        let text = if srv.port == 22 {
            format!("{}  —  {}@{}", srv.name, srv.user, srv.host)
        } else {
            format!("{}  —  {}@{}:{}", srv.name, srv.user, srv.host, srv.port)
        };
        let label = gtk::Label::new(Some(&text));
        label.set_xalign(0.0);
        label.set_margin_top(6);
        label.set_margin_bottom(6);
        label.set_margin_start(8);
        label.set_margin_end(8);
        let row = gtk::ListBoxRow::new();
        row.set_child(Some(&label));
        list_box.append(&row);
    }
    if let Some(first) = list_box.row_at_index(0) {
        list_box.select_row(Some(&first));
    }
}

fn ssh_connect_selected(
    dialog: &gtk::Dialog,
    notebook: &gtk::Notebook,
    list_box: &gtk::ListBox,
    servers: &Rc<RefCell<Vec<SshServer>>>,
) {
    let Some(row) = list_box.selected_row() else { return };
    let index = row.index() as usize;
    let servers_ref = servers.borrow();
    let Some(srv) = servers_ref.get(index) else { return };
    let cmd = if srv.port == 22 {
        format!("ssh {}@{}\n", srv.user, srv.host)
    } else {
        format!("ssh -p {} {}@{}\n", srv.port, srv.user, srv.host)
    };
    let Some(page) = notebook.current_page() else { return };
    let Some(child) = notebook.nth_page(Some(page)) else { return };
    if let Some(terminal) = find_terminal_in_widget(&child) {
        terminal.feed_child(cmd.as_bytes());
    }
    dialog.close();
}

fn show_add_server_dialog(
    parent: &gtk::Dialog,
    list_box: &gtk::ListBox,
    servers: &Rc<RefCell<Vec<SshServer>>>,
) {
    let dialog = gtk::Dialog::new();
    dialog.set_title(Some("Add SSH Server"));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(parent));
    dialog.add_button("Cancel", gtk::ResponseType::Cancel);
    dialog.add_button("Add", gtk::ResponseType::Ok);
    dialog.set_default_response(gtk::ResponseType::Ok);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let grid = gtk::Grid::new();
    grid.set_row_spacing(8);
    grid.set_column_spacing(8);

    let make_label = |text: &str| {
        let l = gtk::Label::new(Some(text));
        l.set_xalign(1.0);
        l
    };
    let make_entry = |placeholder: &str, default: &str| {
        let e = gtk::Entry::new();
        e.set_placeholder_text(Some(placeholder));
        if !default.is_empty() {
            e.set_text(default);
        }
        e.set_activates_default(true);
        e
    };

    let name_entry = make_entry("My Server", "");
    let host_entry = make_entry("192.168.1.1", "");
    let user_entry = make_entry("root", "");
    let port_entry = make_entry("22", "22");

    grid.attach(&make_label("Name:"), 0, 0, 1, 1);
    grid.attach(&name_entry, 1, 0, 1, 1);
    grid.attach(&make_label("Host:"), 0, 1, 1, 1);
    grid.attach(&host_entry, 1, 1, 1, 1);
    grid.attach(&make_label("User:"), 0, 2, 1, 1);
    grid.attach(&user_entry, 1, 2, 1, 1);
    grid.attach(&make_label("Port:"), 0, 3, 1, 1);
    grid.attach(&port_entry, 1, 3, 1, 1);

    content.append(&grid);

    let list_box = list_box.clone();
    let servers = servers.clone();
    dialog.connect_response(move |dialog, response| {
        if response == gtk::ResponseType::Ok {
            let name = name_entry.text().trim().to_string();
            let host = host_entry.text().trim().to_string();
            let user = user_entry.text().trim().to_string();
            let port: u16 = port_entry.text().trim().parse().unwrap_or(22);
            if !name.is_empty() && !host.is_empty() && !user.is_empty() {
                let srv = SshServer { name, host, user, port };
                {
                    let mut svs = servers.borrow_mut();
                    svs.push(srv);
                    save_ssh_servers(&svs);
                }
                populate_server_list(&list_box, &servers.borrow());
            }
        }
        dialog.close();
    });

    dialog.present();
}

fn show_ssh_manager_dialog(window: &gtk::ApplicationWindow, notebook: &gtk::Notebook) {
    let servers = Rc::new(RefCell::new(load_ssh_servers()));

    let dialog = gtk::Dialog::new();
    dialog.set_title(Some("SSH Servers"));
    dialog.set_modal(true);
    dialog.set_transient_for(Some(window));
    dialog.set_default_size(520, 360);

    let content = dialog.content_area();
    content.set_spacing(8);
    content.set_margin_top(12);
    content.set_margin_bottom(12);
    content.set_margin_start(12);
    content.set_margin_end(12);

    let search_entry = gtk::SearchEntry::new();
    search_entry.set_placeholder_text(Some("Ara..."));
    content.append(&search_entry);

    let list_box = gtk::ListBox::new();
    list_box.set_selection_mode(gtk::SelectionMode::Single);
    list_box.set_vexpand(true);

    let scrolled = gtk::ScrolledWindow::new();
    scrolled.set_child(Some(&list_box));
    scrolled.set_vexpand(true);
    scrolled.set_min_content_height(220);
    content.append(&scrolled);

    populate_server_list(&list_box, &servers.borrow());

    // Filtre fonksiyonu
    {
        let search = search_entry.clone();
        list_box.set_filter_func(move |row| {
            let text = search.text().to_lowercase();
            if text.is_empty() { return true; }
            row.child()
                .and_then(|c| c.downcast::<gtk::Label>().ok())
                .is_some_and(|l| l.text().to_lowercase().contains(&text))
        });
    }

    // Arama değiştiğinde filtrele ve ilk görünür satırı seç
    {
        let list_box = list_box.clone();
        search_entry.connect_search_changed(move |_| {
            list_box.invalidate_filter();
            select_first_visible(&list_box);
        });
    }

    // Arama kutusunda ↓ → listeye geç; Escape → dialog'u kapat
    {
        let ctrl = gtk::EventControllerKey::new();
        let list_box = list_box.clone();
        let dialog = dialog.clone();
        ctrl.connect_key_pressed(move |_, key, _, _| match key {
            gdk::Key::Down => {
                if let Some(row) = list_box.selected_row() {
                    row.grab_focus();
                } else {
                    select_first_visible(&list_box);
                    if let Some(row) = list_box.selected_row() {
                        row.grab_focus();
                    }
                }
                gtk::glib::Propagation::Stop
            }
            gdk::Key::Escape => {
                dialog.close();
                gtk::glib::Propagation::Stop
            }
            _ => gtk::glib::Propagation::Proceed,
        });
        search_entry.add_controller(ctrl);
    }

    // Listede ↑ + ilk satır → arama kutusuna dön
    {
        let ctrl = gtk::EventControllerKey::new();
        ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        let search = search_entry.clone();
        let list_box_mv = list_box.clone();
        let list_box_ac = list_box.clone();
        ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Up {
                if let Some(row) = list_box_mv.selected_row() {
                    if Some(row.index()) == first_visible_index(&list_box_mv) {
                        search.grab_focus();
                        return gtk::glib::Propagation::Stop;
                    }
                }
            }
            gtk::glib::Propagation::Proceed
        });
        list_box_ac.add_controller(ctrl);
    }

    let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
    btn_box.set_margin_top(4);
    btn_box.set_halign(gtk::Align::End);

    let add_btn = gtk::Button::with_label("Add");
    let delete_btn = gtk::Button::with_label("Delete");
    let connect_btn = gtk::Button::with_label("Connect");
    let close_btn = gtk::Button::with_label("Close");
    btn_box.append(&add_btn);
    btn_box.append(&delete_btn);
    btn_box.append(&connect_btn);
    btn_box.append(&close_btn);
    content.append(&btn_box);

    // Add button
    {
        let dialog = dialog.clone();
        let list_box = list_box.clone();
        let servers = servers.clone();
        add_btn.connect_clicked(move |_| {
            show_add_server_dialog(&dialog, &list_box, &servers);
        });
    }

    // Delete button
    {
        let list_box = list_box.clone();
        let servers = servers.clone();
        delete_btn.connect_clicked(move |_| {
            let Some(row) = list_box.selected_row() else { return };
            let index = row.index() as usize;
            {
                let mut svs = servers.borrow_mut();
                if index < svs.len() {
                    svs.remove(index);
                }
                save_ssh_servers(&svs);
            }
            populate_server_list(&list_box, &servers.borrow());
            list_box.invalidate_filter();
            select_first_visible(&list_box);
        });
    }

    // Connect button
    {
        let dialog = dialog.clone();
        let notebook = notebook.clone();
        let servers = servers.clone();
        let list_box = list_box.clone();
        connect_btn.connect_clicked(move |_| {
            ssh_connect_selected(&dialog, &notebook, &list_box, &servers);
        });
    }

    // Close button
    {
        let dialog = dialog.clone();
        close_btn.connect_clicked(move |_| dialog.close());
    }

    // Row activated: Enter key or double-click
    {
        let dialog = dialog.clone();
        let notebook = notebook.clone();
        let servers = servers.clone();
        list_box.connect_row_activated(move |lb, _| {
            ssh_connect_selected(&dialog, &notebook, lb, &servers);
        });
    }

    // Delete tuşu seçili sunucuyu siler
    {
        let key_ctrl = gtk::EventControllerKey::new();
        let servers = servers.clone();
        let list_box = list_box.clone();
        key_ctrl.connect_key_pressed(move |_, key, _, _| {
            if key == gdk::Key::Delete {
                let Some(row) = list_box.selected_row() else {
                    return gtk::glib::Propagation::Proceed;
                };
                let index = row.index() as usize;
                {
                    let mut svs = servers.borrow_mut();
                    if index < svs.len() {
                        svs.remove(index);
                    }
                    save_ssh_servers(&svs);
                }
                populate_server_list(&list_box, &servers.borrow());
                list_box.invalidate_filter();
                select_first_visible(&list_box);
                return gtk::glib::Propagation::Stop;
            }
            gtk::glib::Propagation::Proceed
        });
        dialog.add_controller(key_ctrl);
    }

    search_entry.grab_focus();
    dialog.present();
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
        ssh_manager: parse_keybinding("Ctrl+Shift+S").unwrap(),
        password_manager: parse_keybinding("Ctrl+Shift+A").unwrap(),
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
    if let Some(value) = raw.ssh_manager.and_then(|s| parse_keybinding(&s)) {
        bindings.ssh_manager = value;
    }
    if let Some(value) = raw.password_manager.and_then(|s| parse_keybinding(&s)) {
        bindings.password_manager = value;
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
