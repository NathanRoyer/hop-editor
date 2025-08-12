use std::ops::Deref;
use super::*;

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum MenuItem {
    CloseTab,
    NewFile,
    Syntax,
    IndentMode,
    NewDir,
    Rename,
    Delete,
}

impl Deref for MenuItem {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Self::IndentMode => "Indent Mode",
            Self::CloseTab => "Close Tab",
            Self::NewFile => "New File",
            Self::NewDir => "New Dir.",
            Self::Syntax => "Syntax",
            Self::Rename => "Rename",
            Self::Delete => "Delete",
        }
    }
}

fn draw_menu(x: u16, mut y: u16, hover_y: u16, options: &[MenuItem]) {
    let mut stdout = stdout();

    queue!(stdout, MoveTo(x, y)).unwrap();
    write!(stdout, "┌──────────────┐").unwrap();

    for option in options {
        y += 1;

        let bg_color = match hover_y == y {
            true => hover_color(),
            false => default_bg_color(),
        };

        let bg_color = SetBackgroundColor(bg_color);
        let bg_reset = SetBackgroundColor(default_bg_color());

        queue!(stdout, MoveTo(x, y)).unwrap();
        write!(stdout, "│ {bg_color}{:1$}{bg_reset} │", &**option, 12).unwrap();
    }

    y += 1;
    queue!(stdout, MoveTo(x, y)).unwrap();
    write!(stdout, "└──────────────┘").unwrap();

    let _ = stdout.flush();
}

pub fn context_menu(x: u16, y: u16, options: &[MenuItem]) -> Option<MenuItem> {
    let (min_y, max_y) = (y + 1, options.len() as u16 + y);
    let (min_x, max_x) = (x + 1, x + 14);
    set_dirty();

    let mut hover_y = min_y;

    let maybe_index = loop {
        draw_menu(x, y, hover_y, options);

        match read().unwrap() {
            Event::Key(e) if !e.is_release() => match e.code {
                KeyCode::Up => hover_y = hover_y.saturating_sub(1).max(min_y),
                KeyCode::Down => hover_y = hover_y.saturating_add(1).min(max_y),
                KeyCode::Enter => break Some(hover_y),
                KeyCode::Esc => break None,
                _other => (),
            },
            Event::Mouse(e) => {
                let mut hovering = (min_y..=max_y).contains(&e.row);
                hovering &= (min_x..=max_x).contains(&e.column);

                if hovering {
                    hover_y = e.row;
                }

                if e.kind == MouseEventKind::Down(MouseButton::Left) {
                    break hovering.then_some(hover_y);
                }
            },
            _other => (),
        }
    };

    maybe_index.map(|y| options[(y - min_y) as usize])
}
