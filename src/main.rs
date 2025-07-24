use interface::{Interface, ColoredText, TextPart, UserInput};
use syntax::SyntaxFile;
use theme::Theme;
use tab::TabMap;

mod interface;
mod syntax;
mod theme;
// mod tree;
mod tab;

pub struct Globals {
    part_buf: Vec<TextPart>,
    interface: Interface,
    theme: Theme,
    tabs: TabMap,
}

impl Globals {
    fn update_tab_list(&mut self) {
        self.interface.set_tab_list(&self.tabs.tab_list());
    }

    fn update_code(&mut self) {
        for i in 0..self.interface.code_height() {
            let tab = self.tabs.current();

            if let Some(data) = tab.dirty_line(i as usize, &mut self.part_buf) {
                let text = ColoredText::new(&self.part_buf, data.text, &self.theme);
                self.interface.set_code_row(i, data.line_no, text);
            }
        }
    }

    fn run(&mut self) {
        let syntaxes_str = include_str!("../syntax.toml");
        let syntaxes = SyntaxFile::parse(syntaxes_str).unwrap();

        self.tabs.open("src/interface.rs".into()).unwrap();
        let tab = self.tabs.current();
        tab.highlight(&syntaxes);

        self.interface.draw_decorations();

        self.interface.set_tree_row(0, " ▼ opened");
        self.interface.set_tree_row(1, "    ▶ letsgo");
        self.interface.set_tree_row(2, "    ▷ dammit");
        let mut s = String::new();

        loop {
            let tab = self.tabs.current();
            tab.highlight(&syntaxes);

            self.update_tab_list();
            self.update_code();

            let code_height = self.interface.code_height() as isize;
            let tab = self.tabs.current();

            match self.interface.read_event() {
                UserInput::Quit => break,
                UserInput::Insert(c) => {
                    s.clear();
                    s.push(c);
                    tab.insert_text(&s);
                },
                UserInput::Paste => break,
                UserInput::Copy => break,
                UserInput::Cut => break,
                // todo: use bytes instead of chars
                UserInput::Backspace => tab.backspace(1),
                UserInput::ScrollDown(true) => tab.scroll(code_height),
                UserInput::ScrollUp(true) => tab.scroll(-code_height),
                UserInput::ScrollDown(false) => tab.scroll(1),
                UserInput::ScrollUp(false) => tab.scroll(-1),
                UserInput::Resize(_w, _h) => break,
                UserInput::NoOp => (),
            }
        }
    }
}

fn main() -> Result<(), &'static str> {
    let theme_str = include_str!("../theme.toml");

    let mut globals = Globals {
        part_buf: Vec::new(),
        interface: Interface::new(),
        theme: Theme::parse(theme_str)?,
        tabs: TabMap::new(),
    };

    globals.run();

    globals.interface.close();

    Ok(())
}
