use interface::{Interface, ColoredText, TextPart, UserInput, restore_term};
use tab::{TabMap, TabList};
use syntax::SyntaxFile;
use tree::FileTree;
use theme::Theme;

use std::{env, fs, panic, backtrace};

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
    part_buf: Vec<TextPart>,
    list: TabList,

    // singletons
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
        for i in 0..self.interface.code_height() {
            let tab = self.tabs.current();

            if let Some(data) = tab.dirty_line(i, &mut self.part_buf) {
                let text = ColoredText::new(&self.part_buf, data.cursor, data.text, &self.theme);
                self.interface.set_code_row(i, data.line_no, text);
            }
        }
    }

    fn update_tree(&mut self) {
        for i in 0..self.interface.tree_height() {
            let (indent, line) = self.tree.line(i).unwrap_or((0, ""));
            self.interface.set_tree_row(self.tree_hover, i, indent, line, &self.theme);
        }
    }

    fn run(&mut self) {
        let syntaxes_str = include_str!("../syntax.toml");
        let syntaxes = SyntaxFile::parse(syntaxes_str).unwrap();

        self.interface.draw_decorations();
        panic::set_hook(Box::new(panic_handler));

        loop {
            let max_y = self.interface.code_height();
            let tab = self.tabs.current();
            tab.check_overscroll(max_y);
            tab.highlight(&syntaxes);

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
                UserInput::Backspace => {
                    tab.backspace_once();
                    update_list = no_mod;
                },
                UserInput::Scroll(delta) if self.tree_hover.is_none() => tab.scroll(delta),
                UserInput::Scroll(delta) => {
                    self.tree.scroll(delta);
                    let max = self.interface.tree_height();
                    self.tree.check_overscroll(max);
                    update_tree = true;
                },
                UserInput::CodeSeek(x, y) => tab.seek(x, y, false),
                UserInput::TabClick(x) => {
                    if let Some(index) = self.interface.find_tab(x, &self.list) {
                        self.tabs.switch(index);
                        update_list = true;
                    }
                },
                UserInput::TreeClick(y) => {
                    if let Some(path) = self.tree.click(y) {
                        if let Err(err) = self.tabs.open(path) {
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
                UserInput::VerticalJump(d) => tab.vertical_jump(d),
                UserInput::HorizontalJump(d) => tab.horizontal_jump(d),
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

    let mut globals = Globals {
        // state
        list: TabList::new(),
        part_buf: Vec::new(),
        tree_hover: None,
        tab_hover: None,

        // singletons
        interface: Interface::new(),
        tree: FileTree::new(),
        theme: Theme::parse(theme_str)?,
        tabs: TabMap::new(),
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
        } else {
            if let Err(err) = globals.tabs.open(path_str) {
                println!("{err:?}");
                return Err("failed to open some files");
            }
        }
    }

    globals.run();

    globals.interface.close();

    Ok(())
}
