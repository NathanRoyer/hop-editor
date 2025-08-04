use colored_text::{ColoredText, Part as TextPart, Selection};
use interface::{Interface, UserInput, restore_term};
use tab::{TabMap, TabList};
use syntax::SyntaxFile;
use tree::FileTree;

use std::{env, fs, panic, backtrace};
use std::fmt::Write;

mod colored_text;
mod interface;
mod config;
mod syntax;
mod tree;
mod tab;

const CONFIRM_QUIT: &str = "[UNSAVED FILES]\nSome files have unsaved edits!\n- Press Enter to quit.\n- Press Escape to cancel.";
const FIND_PROMPT: &str = "Please input the text to look for.\n- Press Enter to reveal results.\n- Press Escape to cancel.";

const DEFAULT_CONFIG: &str = include_str!("../assets/config.toml");
const DEFAULT_SYNTAX: &str = include_str!("../assets/syntax.toml");

// when refreshing the left panel, this indicates
// we're only doing it to refresh the cursor list.
// in the future, this could be optimized easily.
const FOR_CURSORS: bool = true;
const MAX_CURSORS: u16 = 10;

fn panic_handler(info: &panic::PanicHookInfo) {
    let bt = backtrace::Backtrace::capture();
    confirm!("panic occurred: {info}\n{bt:#?}");
    restore_term();
}

pub struct Globals {
    // state
    tree_select: Option<usize>,
    tree_hover: Option<u16>,
    tab_hover: Option<u16>,
    str_buf: String,
    list: TabList,

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

impl Globals {
    fn update_tab_list(&mut self) {
        let focused = self.tabs.update_tab_list(&mut self.list);
        self.interface.set_tab_list(self.tab_hover, focused, &self.list);
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
                tab::DirtyLine { tab_width_m1: 0, text: "" }
            };

            // cursors are sorted
            let text = ColoredText::new(
                data.tab_width_m1,
                &self.cursor_buf,
                &self.part_buf,
                &self.sel_buf,
                data.text,
            );

            self.interface.set_code_row(i, line_no, text);
        }
    }

    fn update_left(&mut self) {
        let tab = self.tabs.current();

        let height = self.interface.tree_height();
        let num_cursors = tab.cursor_count() as u16;
        let cursor_lines = num_cursors.min(MAX_CURSORS);
        let tree_lines = height.saturating_sub(cursor_lines + 1);
        self.tree.check_overscroll(tree_lines);

        for i in 0..tree_lines {
            let maybe_line = self.tree.line(&mut self.str_buf, i);
            let selected = self.tree_select == maybe_line;
            let hovered = self.tree_hover == Some(i);
            self.interface.set_tree_row(selected, hovered, i, &self.str_buf);
        }

        self.interface.write_cursor_header(tree_lines + 1);

        for i in 0..cursor_lines {
            self.str_buf.clear();
            let y = tree_lines + i + 1;
            tab.cursor_desc(i as usize, &mut self.str_buf);
            self.interface.set_tree_row(false, false, y, &self.str_buf);
        }

        if num_cursors > MAX_CURSORS {
            self.str_buf.clear();
            let y = height.saturating_sub(1);
            let missing = num_cursors - MAX_CURSORS;
            let _ = write!(self.str_buf, "<{missing} other cursors>");
            self.interface.set_tree_row(false, false, y, &self.str_buf);
        }
    }

    fn run(&mut self) {
        self.interface.draw_decorations();
        panic::set_hook(Box::new(panic_handler));

        loop {
            let max_y = self.interface.code_height();
            let tab = self.tabs.current();
            tab.check_overscroll(max_y);

            if self.interface.must_refresh() {
                self.interface.draw_decorations();
                tab.set_fully_dirty();
                self.update_tab_list();
                self.update_left();
            }

            self.update_code();
            let tab = self.tabs.current();
            let no_mod = !tab.modified();
            let mut update_list = false;
            let mut update_left = false;

            match (self.tree_select.as_mut(), self.interface.read_event()) {
                (Some(_), UserInput::Quit(false)) => {
                    self.tree_select = None;
                    update_left = true;
                },
                (None, UserInput::Quit(false)) if tab.has_selections() => {
                    tab.horizontal_jump(0, false);
                },
                (_, UserInput::Quit(_)) => match self.tabs.all_saved() {
                    true => break,
                    false if confirm!("{}", CONFIRM_QUIT) => break,
                    _otherwise => (/* do not quit */),
                },
                (_, UserInput::Save) => {
                    tab.save();
                    update_list = true;
                },
                (None, UserInput::Insert(c)) => {
                    tab.insert_char(c);
                    update_list = no_mod;
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Paste) => {
                    tab.paste();
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Copy) => tab.copy(),
                (None, UserInput::Cut) => {
                    tab.cut();
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Backspace(forward)) => {
                    tab.backspace_once(forward);
                    update_list = no_mod;
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Scroll(delta)) if self.tree_hover.is_none() => tab.scroll(delta),
                (_, UserInput::Scroll(delta)) => {
                    self.tree.scroll(delta);
                    update_left = true;
                },
                (_, UserInput::CodeSeek(x, y, push_c)) => {
                    // update_left = self.tree_select.take().is_some();
                    update_left = FOR_CURSORS;
                    tab.seek(x, y, push_c);
                },
                (None, UserInput::CodeDrag(x, y)) => {
                    tab.drag_to(x, y);
                    update_left = FOR_CURSORS;
                },
                (_, UserInput::TabClick(x)) => {
                    if let Some(index) = self.interface.find_tab(x, &self.list) {
                        self.tree_select.take();
                        self.tabs.switch(index);
                        update_list = true;
                        update_left = FOR_CURSORS;
                    }
                },
                (_, UserInput::TreeClick(y)) => {
                    if let Some(path) = self.tree.click(y) {
                        if let Err(err) = self.tabs.open(&self.syntaxes, path) {
                            confirm!("failed to open: {err:?}");
                        }
                    }

                    self.tree_select.take();
                    update_list = true;
                    update_left = true;
                },
                (_, UserInput::CloseTab) => {
                    self.tabs.close(None);
                    update_left = FOR_CURSORS;
                    update_list = true;
                },
                (_, UserInput::NextTab(leftward)) => {
                    self.tabs.next_tab(leftward);
                    update_left = FOR_CURSORS;
                    update_list = true;
                },
                (_, UserInput::TreeHover(y)) => {
                    update_left = self.tree_hover != Some(y);
                    update_list = self.tab_hover.take().is_some();
                    self.tree_hover = Some(y);
                },
                (_, UserInput::TabHover(x)) => {
                    update_list = self.tab_hover != Some(x);
                    update_left = self.tree_hover.take().is_some();
                    self.tab_hover = Some(x);
                },
                (_, UserInput::ClearHover) => {
                    update_left = self.tree_hover.take().is_some();
                    update_list = self.tab_hover.take().is_some();
                },
                (_, UserInput::Reveal) => {
                    let path = tab.path().unwrap_or("");
                    let index = self.tree.reveal_path(path).unwrap_or(0);
                    self.tree_select = Some(index);
                    update_left = true;
                },
                (None, UserInput::HorizontalJump(d, s)) => {
                    tab.horizontal_jump(d, s);
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::VerticalJump(d, s)) => {
                    tab.vertical_jump(d, s);
                    update_left = FOR_CURSORS;
                },
                (Some(i), UserInput::HorizontalJump(-1, false)) => {
                    self.tree.leave_dir(i);
                    update_left = true;
                },
                (Some(i), UserInput::HorizontalJump(1, false)) => {
                    self.tree.enter_dir(i);
                    update_left = true;
                },
                (Some(i), UserInput::VerticalJump(d, false)) => {
                    self.tree.up_down(i, d);
                    update_left = true;
                },
                (Some(i), UserInput::Insert('\n' | ' ')) => {
                    if let Some(path) = self.tree.toggle_or_open(*i) {
                        if let Err(err) = self.tabs.open(&self.syntaxes, path) {
                            confirm!("failed to open: {err:?}");
                        }
                    }

                    update_left = true;
                    update_list = true;
                },
                (_, UserInput::Resize(w, h)) => self.interface.resize(w, h),
                (None, UserInput::AutoSelect) => {
                    tab.auto_select();
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Undo) => {
                    tab.undo();
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Redo) => {
                    tab.redo();
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::Find) => {
                    if let Some(text) = prompt!("{}", FIND_PROMPT) {
                        tab.find_all(&text);
                        update_left = FOR_CURSORS;
                    }
                },
                (None, UserInput::SeekLineStart(s)) => {
                    tab.line_seek(true, s);
                    update_left = FOR_CURSORS;
                },
                (None, UserInput::SeekLineEnd(s)) => {
                    tab.line_seek(false, s);
                    update_left = FOR_CURSORS;
                },
                _others => (),
            }

            if update_left {
                self.update_left();
            }

            if update_list {
                self.update_tab_list();
            }
        }
    }
}

fn main() -> Result<(), &'static str> {
    config::init();

    let syntaxes = config::syntax_file();
    let mut interface = Interface::new();
    interface.draw_decorations();

    let mut globals = Globals {
        // state
        str_buf: String::new(),
        cursor_buf: Vec::new(),
        list: TabList::new(),
        part_buf: Vec::new(),
        sel_buf: Vec::new(),
        tree_select: None,
        tree_hover: None,
        tab_hover: None,

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
            globals.tree.add_folder(path_str);
        } else if let Err(err) = globals.tabs.open(&globals.syntaxes, path_str) {
            restore_term();
            println!("{err:?}");
            return Err("failed to open some files");
        }
    }

    globals.run();
    globals.interface.close();

    Ok(())
}
