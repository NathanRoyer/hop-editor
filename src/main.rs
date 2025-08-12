use interface::colored_text::{ColoredText, Part as TextPart, Selection};
use interface::input::{UserInput, ResizeEvent, Location};
use interface::menu::{MenuItem, context_menu};
use interface::{Interface, restore_term};
use tab::{TabMap, TabList};
use syntax::SyntaxFile;
use tree::FileTree;

use std::{env, fs, panic, backtrace};
use std::fmt::Write;

mod interface;
mod config;
mod syntax;
mod tree;
mod tab;

const CONFIRM_QUIT: &str = "[UNSAVED FILES]\nReally Quit? Some files have unsaved edits!";

const DEFAULT_CONFIG: &str = include_str!("../assets/config.toml");
const DEFAULT_SYNTAX: &str = include_str!("../assets/syntax.toml");

// when refreshing the left panel, this indicates
// we're only doing it to refresh the cursor list.
// in the future, this could be optimized easily.
const FOR_CURSORS: bool = true;

// ⚠

fn panic_handler(info: &panic::PanicHookInfo) {
    let bt = backtrace::Backtrace::capture();
    alert!("panic occurred: {info}\n{bt:#?}");
    restore_term();
}

pub struct Application {
    // state
    cursor_hover: Option<u16>,
    tree_select: Option<usize>,
    fallback_panel_width: u16,
    tree_hover: Option<u16>,
    tab_hover: Option<u16>,
    cursor_list_scroll: u16,
    shown_cursors: u16,
    max_cursor_scroll: u16,
    str_buf: String,
    list: TabList,
    stop: bool,

    // these three should stay sorted
    part_buf: Vec<TextPart>,
    sel_buf: Vec<Selection>,
    cursor_buf: Vec<usize>,

    // singletons
    syntaxes: SyntaxFile,
    interface: Interface,
    tree: FileTree,
    tabs: TabMap,
}

impl Application {
    fn update_tab_list(&mut self, actually: bool) {
        if actually {
            let focused = self.tabs.update_tab_list(&mut self.list);
            self.interface.set_tab_list(self.tab_hover, focused, &self.list);
        }
    }

    fn update_code(&mut self) {
        let tab = self.tabs.current();
        tab.highlight();

        for i in 0..self.interface.code_height() {
            let mut line_no = None;
            self.cursor_buf.clear();
            self.part_buf.clear();
            self.sel_buf.clear();

            let data = if let Some((index, dirty)) = tab.prepare_draw(i) {
                if !dirty {
                    continue;
                }

                line_no = Some(index + 1);
                tab.line_data(index, &mut self.part_buf, &mut self.sel_buf, &mut self.cursor_buf)
            } else {
                tab::DirtyLine { horizontal_scroll: 0, tab_width_m1: 0, text: "" }
            };

            // cursors are sorted
            let text = ColoredText::new(
                data.horizontal_scroll,
                data.tab_width_m1,
                &self.cursor_buf,
                &self.part_buf,
                &self.sel_buf,
                data.text,
            );

            self.interface.set_code_row(i, line_no, text);
        }
    }

    fn update_left(&mut self, actually: bool) {
        if !actually {
            return;
        }

        let tab = self.tabs.current();
        let max_cursors = config::max_cursor_lines();

        let height = self.interface.tree_height();
        let num_cursors = tab.cursor_count() as u16;
        let cursor_lines = num_cursors.min(max_cursors);
        let tree_lines = height.saturating_sub(cursor_lines + 1);
        self.tree.check_overscroll();

        if self.shown_cursors != cursor_lines {
            self.shown_cursors = cursor_lines;
            self.max_cursor_scroll = num_cursors.saturating_sub(max_cursors);
            self.cursor_list_scroll = 0;
        }

        for i in 0..tree_lines {
            self.str_buf.clear();
            let maybe_line = self.tree.line(&mut self.str_buf, i);
            let selected = self.tree_select == maybe_line;
            let hovered = self.tree_hover == Some(i);
            self.interface.set_tree_row(selected, hovered, i, &self.str_buf);
        }

        self.str_buf.clear();
        let _ = write!(self.str_buf, " Cursors: {num_cursors} ");
        self.interface.write_header(tree_lines + 1, &self.str_buf);

        for i in 0..cursor_lines {
            self.str_buf.clear();
            let y = tree_lines + i + 1;
            let c = (i + self.cursor_list_scroll) as usize;
            tab.cursor_desc(c, &mut self.str_buf);
            let hovered = self.cursor_hover == Some(i);
            self.interface.set_tree_row(false, hovered, y, &self.str_buf);
        }
    }

    fn quit(&mut self, with_ctrl: bool) {
        if !with_ctrl {
            let tab = self.tabs.current();

            if self.tree_select.is_some() {
                self.tree_select = None;
                self.update_left(true);
                return;
            } else if tab.has_selections() {
                tab.horizontal_jump(0, false);
                return;
            }
        }

        self.stop = match self.tabs.all_saved() {
            true => true,
            false => confirm!("{}", CONFIRM_QUIT),
        };
    }

    fn scroll(&mut self, delta: isize) {
        if self.tree_hover.is_some() || self.tree_select.is_some() {
            self.tree.scroll(delta);
            self.update_left(true);
        } else if self.cursor_hover.is_some() {
            let n = &mut self.cursor_list_scroll;

            match n.checked_add_signed(delta as i16) {
                None => *n = 0,
                Some(s) if s <= self.max_cursor_scroll => *n = s,
                Some(_) => *n = self.max_cursor_scroll,
            };

            self.update_left(true);
        } else {
            self.tabs.current().scroll(delta);
        }
    }

    fn ensure_cursor_visible(&mut self) {
        let h = self.interface.code_height() as usize;
        let w = self.interface.code_width();
        let tab = self.tabs.current();
        tab.ensure_cursor_visible(w, h);
    }

    fn tree_toggle(&mut self, i: usize) {
        if let Some(path) = self.tree.toggle_or_open(i) {
            if let Err(err) = self.tabs.open(&self.syntaxes, path) {
                alert!("failed to open: {err:?}");
            }
        }

        self.update_left(true);
        self.update_tab_list(true);
    }

    fn carriage_return(&mut self) {
        if let Some(i) = self.tree_select {
            return self.tree_toggle(i);
        }

        let tab = self.tabs.current();
        let no_mod = !tab.modified();
        tab.smart_carriage_return();
        self.ensure_cursor_visible();
        self.update_tab_list(no_mod);
        self.update_left(FOR_CURSORS);
    }

    fn insert(&mut self, c: char) {
        if let (' ', Some(i)) = (c, self.tree_select) {
            return self.tree_toggle(i);
        }

        let tab = self.tabs.current();
        let no_mod = !tab.modified();
        tab.insert_char(c);
        self.ensure_cursor_visible();
        self.update_tab_list(no_mod);
        self.update_left(FOR_CURSORS);
    }

    fn horizontal_jump(&mut self, delta: isize, shift: bool) {
        if let Some(i) = self.tree_select.as_mut() {
            match delta < 0 {
                true => self.tree.leave_dir(i),
                false => self.tree.enter_dir(i),
            }

            self.update_left(true);
        } else {
            let tab = self.tabs.current();
            tab.horizontal_jump(delta, shift);
            self.ensure_cursor_visible();
            self.update_left(FOR_CURSORS);
        }
    }

    fn vertical_jump(&mut self, delta: isize, shift: bool) {
        if let Some(i) = self.tree_select.as_mut() {
            if !shift {
                self.tree.up_down(i, delta);
                self.update_left(true);
            }
        } else {
            let tab = self.tabs.current();
            tab.vertical_jump(delta, shift);
            self.ensure_cursor_visible();
            self.update_left(FOR_CURSORS);
        }
    }

    fn tab_menu(&mut self, col: u16, x: u16, y: u16) {
        use MenuItem::*;

        let Some(index) = self.interface.find_tab(col, &self.list) else {
            return;
        };

        let tab = self.tabs.get_mut(index);
        let actions = [CloseTab, Syntax, IndentMode];
        let close = UserInput::CloseTab(Some(col));

        match context_menu(x, y, &actions) {
            Some(CloseTab) => self.handle_event(close),
            Some(Syntax) => tab.switch_syntax(&self.syntaxes),
            Some(IndentMode) => tab.switch_indent_mode(),
            _ => (),
        }
    }

    fn handle_tab_event(&mut self, event: UserInput) {
        if self.tree_select.is_some() {
            return;
        };

        let tab = self.tabs.current();
        let mut ensure_cursor_visible = false;
        let mut update_tab_list = !tab.modified();

        // all of these should update tab list
        match event {
            UserInput::Paste => tab.paste(),
            UserInput::Cut => tab.cut(),
            UserInput::InsertTab => tab.insert_tab(),
            UserInput::Backspace(forward) => tab.backspace_once(forward),
            _ => update_tab_list = false,
        }

        // none of these should make cursor visible
        match event {
            UserInput::Copy => tab.copy(),
            UserInput::SelectAll => tab.select_all(),
            UserInput::Find => tab.find_all(),
            _ => ensure_cursor_visible = true,
        };

        match event {
            UserInput::Undo => tab.undo(),
            UserInput::Redo => tab.redo(),
            UserInput::AutoSelect => tab.auto_select(),
            UserInput::SeekLineStart(s) => tab.line_seek(true, s),
            UserInput::SeekLineEnd(s) => tab.line_seek(false, s),
            UserInput::CodeDrag(x, y) => tab.drag_to(x, y),
            _other => (),
        }

        if ensure_cursor_visible {
            self.ensure_cursor_visible();
        }

        self.update_tab_list(update_tab_list);
        self.update_left(FOR_CURSORS);
    }

    fn resize_left_panel(&mut self, toggle: bool) {
        if toggle {
            let fallback = self.fallback_panel_width;

            let op = move |n| match n {
                0 => fallback,
                _ => 0,
            };

            self.interface.panel_width_op(&op);
            interface::set_dirty();
            return;
        }

        loop {
            let op: &dyn Fn(u16) -> u16 = match self.interface.panel_resize_event() {
                ResizeEvent::Drag(y) => &move |_| y,
                ResizeEvent::Right => &|n| n.saturating_add(1),
                ResizeEvent::Left => &|n| n.saturating_sub(1),
                ResizeEvent::Stop => break,
                ResizeEvent::NoOp => &|n| n,
            };

            self.interface.panel_width_op(op);
            self.interface.draw_decorations();
            self.update_tab_list(true);
            self.update_left(true);
        }

        self.fallback_panel_width = self.interface.get_panel_width();
        self.tabs.current().set_fully_dirty();
    }

    fn handle_event(&mut self, event: UserInput) {
        let tab = self.tabs.current();

        match event {
            UserInput::NoOp => (),
            UserInput::Quit(with_ctrl) => self.quit(with_ctrl),
            UserInput::PanelResize(toggle) => self.resize_left_panel(toggle),
            UserInput::Save => {
                tab.save();
                self.update_tab_list(true);
            },
            UserInput::ContextMenu(Location::TreeRow(row), x, y) => {
                let is_in_use = |p: &str| self.tabs.is_in_use(p);
                self.tree.right_click(x, y, row, is_in_use);
                self.update_left(true);
            },
            UserInput::ContextMenu(Location::Tab(col), x, y) => self.tab_menu(col, x, y),
            UserInput::CodeSeek(x, y, push_c) => {
                tab.seek(x, y, push_c);
                let _tree_select = self.tree_select.take();
                // self.update_left(_tree_select.is_some());
                self.update_left(FOR_CURSORS);
                self.ensure_cursor_visible();
            },
            UserInput::TabClick(x) => {
                if let Some(index) = self.interface.find_tab(x, &self.list) {
                    self.tree_select.take();
                    self.tabs.switch(index);
                    self.update_tab_list(true);
                    self.update_left(FOR_CURSORS);
                }
            },
            UserInput::TreeClick(y) => {
                if let Some(path) = self.tree.click(y) {
                    if let Err(err) = self.tabs.open(&self.syntaxes, path) {
                        alert!("failed to open: {err:?}");
                    }
                }

                self.tree_select.take();
                self.update_tab_list(true);
                self.update_left(true);
            },
            UserInput::CursorClick(y) => {
                let y = self.cursor_list_scroll + y;
                tab.swap_latest_cursor(y as usize);
                self.ensure_cursor_visible();
            },
            UserInput::CloseTab(None) => {
                self.tabs.close(None);
                self.update_left(FOR_CURSORS);
                self.update_tab_list(true);
            },
            UserInput::CloseTab(Some(x)) => {
                if let Some(index) = self.interface.find_tab(x, &self.list) {
                    self.tabs.close(Some(index));
                    self.update_left(FOR_CURSORS);
                    self.update_tab_list(true);
                }
            },
            UserInput::NextTab(leftward) => {
                self.tabs.next_tab(leftward);
                self.update_left(FOR_CURSORS);
                self.update_tab_list(true);
            },
            UserInput::TabHover(x) => {
                let update_list = self.tab_hover != Some(x);
                let cursor_hover = self.cursor_hover.take();
                let tree_hover = self.tree_hover.take();
                self.tab_hover = Some(x);
                self.update_left(tree_hover.is_some() | cursor_hover.is_some());
                self.update_tab_list(update_list);
            },
            UserInput::TreeHover(y) => {
                let update_left = self.tree_hover != Some(y);
                let tab_hover = self.tab_hover.take();
                self.tree_hover = Some(y);
                self.update_left(update_left);
                self.update_tab_list(tab_hover.is_some());
            },
            UserInput::CursorHover(y) => {
                let update_left = self.cursor_hover != Some(y);
                let tab_hover = self.tab_hover.take();
                self.cursor_hover = Some(y);
                self.update_left(update_left);
                self.update_tab_list(tab_hover.is_some());
            },
            UserInput::ClearHover => {
                let cursor_hover = self.cursor_hover.take();
                let tree_hover = self.tree_hover.take();
                let tab_hover = self.tab_hover.take();
                self.update_left(tree_hover.is_some() | cursor_hover.is_some());
                self.update_tab_list(tab_hover.is_some());
            },
            UserInput::Reveal => {
                let path = tab.path().unwrap_or("");
                let index = self.tree.reveal_path(path).unwrap_or(0);
                self.tree_select = Some(index);
                self.update_left(true);
            },
            UserInput::CarriageReturn => self.carriage_return(),
            UserInput::HorizontalJump(d, s) => self.horizontal_jump(d, s),
            UserInput::VerticalJump(d, s) => self.vertical_jump(d, s),
            UserInput::Resize(w, h) => self.interface.resize(w, h),
            UserInput::Scroll(delta) => self.scroll(delta),
            UserInput::Insert(c) => self.insert(c),
            other => self.handle_tab_event(other),
        }
    }

    fn run(&mut self) {
        self.interface.draw_decorations();
        panic::set_hook(Box::new(panic_handler));

        while !self.stop {
            let tab = self.tabs.current();
            tab.check_overscroll();

            if self.interface.must_refresh() {
                self.interface.draw_decorations();
                tab.set_fully_dirty();
                self.update_tab_list(true);
                self.update_left(true);
            }

            self.update_code();

            let event = self
                .interface
                .read_event(self.shown_cursors);

            self.handle_event(event);
        }
    }
}

fn main() -> Result<(), &'static str> {
    config::init();

    let fallback_panel_width = config::tree_width();
    let syntaxes = config::syntax_file();
    let mut interface = Interface::new();
    interface.draw_decorations();

    let mut app = Application {
        // state
        str_buf: String::new(),
        cursor_buf: Vec::new(),
        fallback_panel_width,
        list: TabList::new(),
        cursor_list_scroll: 0,
        max_cursor_scroll: 0,
        part_buf: Vec::new(),
        sel_buf: Vec::new(),
        cursor_hover: None,
        tree_select: None,
        tree_hover: None,
        tab_hover: None,
        shown_cursors: 0,
        stop: false,

        // singletons
        interface,
        tree: FileTree::new(),
        tabs: TabMap::new(),
        syntaxes,
    };

    let mut args = env::args();
    let _this = args.next();

    for arg in args {
        let Ok(path) = fs::canonicalize(arg) else {
            restore_term();
            return Err("invalid path");
        };

        let Some(path_str) = path.to_str().map(String::from) else {
            restore_term();
            return Err("invalid path");
        };

        if path.is_dir() {
            app.tree.add_folder(path_str);
        } else if let Err(err) = app.tabs.open(&app.syntaxes, path_str) {
            restore_term();
            println!("{err:?}");
            return Err("failed to open some files");
        }
    }

    app.run();
    app.interface.close();

    Ok(())
}
