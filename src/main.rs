use interface::{Interface, ColoredText};
use theme::Theme;
use tab::TabMap;

mod interface;
mod syntax;
mod theme;
mod tree;
mod tab;

pub struct Globals {
    interface: Interface,
    theme: Theme,
    tabs: TabMap,
}

impl Globals {
    fn update_tab_list(&mut self) {
        self.interface.set_tab_list(&self.tabs.tab_list());
    }
}

fn main() -> Result<(), &'static str> {
    let theme_str = include_str!("../theme.toml");

    let mut globals = Globals {
        interface: Interface::new(),
        theme: Theme::parse(theme_str)?,
        tabs: TabMap::new(),
    };

    globals.interface.draw_decorations();
    globals.update_tab_list();

    globals.interface.set_tree_row(0, " ▼ opened");
    globals.interface.set_tree_row(1, "    ▶ letsgo");
    globals.interface.set_tree_row(2, "    ▷ dammit");

    let text = ColoredText::new(&[("ilower", "test 1")], &globals.theme);
    globals.interface.set_code_row(2, 2, text);

    let text = ColoredText::new(&[("clower", "test 2")], &globals.theme);
    globals.interface.set_code_row(3, 3, text);

    globals.interface.read_events();

    globals.interface.close();

    Ok(())
}
