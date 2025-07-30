use colored_text::{ColoredText, Part as TextPart, Selection};
use interface::{Interface, UserInput, restore_term};
use tab::{TabMap, TabList};
use syntax::SyntaxFile;
use tree::FileTree;
use theme::Theme;

use std::{env, fs, panic, backtrace};

mod colored_text;
mod interface;
mod syntax;
mod theme;
mod tree;
mod tab;

const CONFIRM_QUIT: &str = "[UNSAVED FILES]\nSome files have unsaved edits!\n- Press Enter to quit.\n- Press Escape to cancel.";

fn panic_handler(info: &panic::PanicHookInfo) {
    let bt = backtrace::Backtrace::capture();
    confirm!("panic occurred: {info}\n{bt:#?}");
    restore_term();
}

pub struct Globals {
    // state
    tree_hover: Option<u16>,
    tab_hover: Option<u16>,
    list: TabList,

    // these three should stay sorted
    part_buf: Vec<TextPart>,
    sel_buf: Vec<Selection>,
    cursor_buf: Vec<usize>,

    // singletons
    syntaxes: SyntaxFile,
    interface: Interface,
    tree: FileTree,
    theme: Theme,
    tabs: TabMap,
}

impl Globals {
    fn update_tab_list(&mut self) {
        let focused = self.tabs.update_tab_list(&mut self.list);
        self.interface.set_tab_list(self.tab_hover, focused, &self.list, &self.theme);
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
                &self.theme,
                data.text,
            );

            self.interface.set_code_row(i, line_no, text);
        }
    }

    fn update_tree(&mut self) {
        let mut buf = String::with_capacity(64);
        for i in 0..self.interface.tree_height() {
            self.tree.line(&mut buf, i);
            self.interface.set_tree_row(self.tree_hover, i, &buf, &self.theme);
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
                self.update_tree();
            }

            self.update_code();
            let tab = self.tabs.current();
            let no_mod = !tab.modified();
            let mut update_list = false;
            let mut update_tree = false;

            match self.interface.read_event() {
                UserInput::Quit => match self.tabs.all_saved() {
                    true => break,
                    false if confirm!("{}", CONFIRM_QUIT) => break,
                    _otherwise => (/* do not quit */),
                },
                UserInput::Insert(c) => {
                    tab.insert_char(c);
                    update_list = no_mod;
                },
                UserInput::Paste => break,
                UserInput::Copy => break,
                UserInput::Cut => break,
                UserInput::Save => {
                    tab.save();
                    update_list = true;
                },
                UserInput::Backspace(forward) => {
                    tab.backspace_once(forward);
                    update_list = no_mod;
                },
                UserInput::Scroll(delta) if self.tree_hover.is_none() => tab.scroll(delta),
                UserInput::Scroll(delta) => {
                    self.tree.scroll(delta);
                    let max = self.interface.tree_height();
                    self.tree.check_overscroll(max);
                    update_tree = true;
                },
                UserInput::CodeSeek(x, y, push_c) => tab.seek(x, y, push_c),
                UserInput::CodeDrag(x, y) => tab.drag_to(x, y),
                UserInput::ClearDrag => (),
                UserInput::TabClick(x) => {
                    if let Some(index) = self.interface.find_tab(x, &self.list) {
                        self.tabs.switch(index);
                        update_list = true;
                    }
                },
                UserInput::TreeClick(y) => {
                    if let Some(path) = self.tree.click(y) {
                        if let Err(err) = self.tabs.open(&self.syntaxes, path) {
                            confirm!("failed to open: {err:?}");
                        }
                    }

                    update_list = true;
                    update_tree = true;
                },
                UserInput::CloseTab => {
                    self.tabs.close(None);
                    update_list = true;
                },
                UserInput::NextTab(leftward) => {
                    self.tabs.next_tab(leftward);
                    update_list = true;
                },
                UserInput::TreeHover(y) => {
                    update_tree = self.tree_hover != Some(y);
                    update_list = self.tab_hover.take().is_some();
                    self.tree_hover = Some(y);
                },
                UserInput::TabHover(x) => {
                    update_list = self.tab_hover != Some(x);
                    update_tree = self.tree_hover.take().is_some();
                    self.tab_hover = Some(x);
                },
                UserInput::ClearHover => {
                    update_tree = self.tree_hover.take().is_some();
                    update_list = self.tab_hover.take().is_some();
                },
                UserInput::HorizontalJump(d, s) => tab.horizontal_jump(d, s),
                UserInput::VerticalJump(d, s) => tab.vertical_jump(d, s),
                UserInput::Resize(w, h) => self.interface.resize(w, h),
                UserInput::NoOp => (),
            }

            if update_tree {
                self.update_tree();
            }

            if update_list {
                self.update_tab_list();
            }
        }
    }
}

fn main() -> Result<(), &'static str> {
    let theme_str = include_str!("../theme.toml");
    let syntaxes_str = include_str!("../syntax.toml");
    let syntaxes = SyntaxFile::parse(syntaxes_str).unwrap();

    let mut globals = Globals {
        // state
        cursor_buf: Vec::new(),
        list: TabList::new(),
        part_buf: Vec::new(),
        sel_buf: Vec::new(),
        tree_hover: None,
        tab_hover: None,

        // singletons
        interface: Interface::new(),
        tree: FileTree::new(),
        theme: Theme::parse(theme_str)?,
        tabs: TabMap::new(),
        syntaxes,
    };

    let mut args = env::args();
    let _this = args.next();

    for arg in args {
        let Ok(path) = fs::canonicalize(arg) else {
            return Err("invalid path");
        };

        let Some(path_str) = path.to_str().map(String::from) else {
            return Err("invalid path");
        };

        if path.is_dir() {
            globals.tree.add_folder(path_str);
        } else if let Err(err) = globals.tabs.open(&globals.syntaxes, path_str) {
            println!("{err:?}");
            return Err("failed to open some files");
        }
    }

    globals.run();

    globals.interface.close();

    Ok(())
}
