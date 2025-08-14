use crossterm::{*, terminal::*, event::*, cursor::*, style::*};
use std::io::{stdout, Stdout, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fmt::{self, Write as _};
use std::mem::take;

use colored_text::ColoredText;

use crate::config::{ansi_color, tree_width, default_bg_color, hover_color};
use crate::tab::TabList;

pub mod colored_text;
pub mod popup;
pub mod input;
pub mod menu;

const TABS_HEIGHT: u16 = 3;
const MENU_HEIGHT: u16 = 4;
const LN_WIDTH: usize = 4;

static DIRTY: AtomicBool = AtomicBool::new(true);

pub struct Interface {
    str_buf: String,
    stdout: Stdout,
    panel_width: u16,
    height: u16,
    width: u16,
}

impl Interface {
    pub fn new() -> Self {
        let mut stdout = stdout();
        queue!(stdout, SavePosition).unwrap();
        queue!(stdout, EnterAlternateScreen).unwrap();
        queue!(stdout, EnableMouseCapture).unwrap();
        queue!(stdout, SetBackgroundColor(default_bg_color())).unwrap();
        queue!(stdout, Hide).unwrap();
        let _ = enable_raw_mode();
        let _ = stdout.flush();

        let (width, height) = size().unwrap();

        Self {
            str_buf: String::with_capacity(1024),
            panel_width: tree_width(),
            stdout,
            height,
            width,
        }
    }

    pub fn must_refresh(&self) -> bool {
        DIRTY.swap(false, Ordering::SeqCst)
    }

    pub fn resize(&mut self, w: u16, h: u16) {
        self.width = w;
        self.height = h;
        set_dirty();
    }

    pub fn close(self) {
        restore_term();
    }

    pub fn tree_height(&self) -> u16 {
        self.height.saturating_sub(MENU_HEIGHT + 1)
    }

    pub fn code_height(&self) -> u16 {
        self.height.saturating_sub(TABS_HEIGHT)
    }

    fn tabs_width(&self) -> usize {
        self.width.saturating_sub(self.panel_width + 1).into()
    }

    pub fn code_width(&self) -> usize {
        self.tabs_width().saturating_sub(LN_WIDTH + 2)
    }

    fn erase_tab_list(&mut self, offset: u16) {
        let x = self.panel_width + 1 + offset;
        let len = self.tabs_width().saturating_sub(offset.into());

        queue!(self.stdout, MoveTo(x, 0)).unwrap();
        let _ = write!(self.stdout, "{:╌^1$}", "", len);

        queue!(self.stdout, MoveTo(x, 1)).unwrap();
        let _ = write!(self.stdout, "{:1$}", "", len);

        queue!(self.stdout, MoveTo(x, 2)).unwrap();
        let _ = write!(self.stdout, "{:╌^1$}", "", len);
    }

    pub fn draw_decorations(&mut self) {
        queue!(self.stdout, SetBackgroundColor(default_bg_color())).unwrap();
        queue!(self.stdout, Clear(ClearType::All)).unwrap();
        self.write_header(0, " Folders ");

        for y in 0..self.height {
            queue!(self.stdout, MoveTo(self.panel_width, y)).unwrap();

            let c = match y {
                0 | 2 => '├',
                MENU_HEIGHT => '┤',
                _ => '│',
            };

            let _ = write!(self.stdout, "{}", c);
        }

        self.erase_tab_list(0);

        let _ = self.stdout.flush();
    }

    pub fn write_text<T: fmt::Display>(&mut self, x: u16, y: u16, text: T) {
        queue!(self.stdout, SetForegroundColor(Color::Reset)).unwrap();
        queue!(self.stdout, MoveTo(x, y)).unwrap();
        let _ = write!(self.stdout, "{text}");
        let _ = self.stdout.flush();
    }

    pub fn set_tree_row(
        &mut self,
        selected: bool,
        hovered: bool,
        index: u16,
        text: &str,
    ) {
        if hovered {
            let color = hover_color();
            queue!(self.stdout, SetBackgroundColor(color)).unwrap();
        }

        if selected {
            queue!(self.stdout, SetAttribute(Attribute::Reverse)).unwrap();
        }

        let max = self.panel_width.min(self.width) as usize;
        let (cut, chars) = cut_len(text, max);
        self.write_text(0, MENU_HEIGHT + 1 + index, &text[..cut]);

        queue!(self.stdout, SetBackgroundColor(default_bg_color())).unwrap();
        queue!(self.stdout, SetAttribute(Attribute::NoReverse)).unwrap();
        write!(self.stdout, "{:1$}│", "", max.saturating_sub(chars)).unwrap();

        let _ = self.stdout.flush();
    }

    pub fn write_header(&mut self, y: u16, mut text: &str) {
        queue!(self.stdout, SetForegroundColor(Color::Reset)).unwrap();
        queue!(self.stdout, MoveTo(0, MENU_HEIGHT + y)).unwrap();
        let width = self.panel_width as usize;

        if width < text.chars().count() {
            text = "";
        }

        let _ = write!(self.stdout, "{:─^1$}┤", text, width);
    }

    pub fn set_code_row(&mut self, index: u16, line_no: Option<usize>, mut text: ColoredText) {
        let line_no: &dyn fmt::Display = match line_no.as_ref() {
            Some(n) => n,
            None => &"",
        };

        let mut buf = take(&mut self.str_buf);
        buf.clear();
        let _ = write!(&mut buf, "{:1$} ", line_no, LN_WIDTH);

        let y = TABS_HEIGHT + index;
        let mut x = self.panel_width + 1;
        self.write_text(x, y, &buf);

        x += LN_WIDTH as u16 + 2;
        text.set_max(self.width.saturating_sub(x) as usize);
        self.write_text(x, y, text);
        let _ = queue!(self.stdout, Clear(ClearType::UntilNewLine));

        self.str_buf = buf;
        let _ = self.stdout.flush();
    }

    pub fn set_tab_list(
        &mut self,
        hover_pos: Option<u16>,
        focused: usize,
        items: &TabList,
    ) {
        let tabs = self.panel_width + 1;
        let mut cursor = 0;

        for (i, (modified, tab_name)) in items.iter().enumerate() {
            queue!(self.stdout, MoveTo(tabs + cursor, 1)).unwrap();
            let cells = tab_name.chars().count() + 4;
            let mut hovered = false;

            if let Some(pos) = hover_pos {
                hovered = match pos.checked_sub(cursor) {
                    Some(rem) => (rem as usize) < cells,
                    None => false,
                };
            }

            let bg_color = match hovered {
                true => hover_color(),
                false => default_bg_color(),
            };

            let fg_color = match (i == focused, modified) {
                (true, false) => ansi_color("kw-strong"),
                (true, true) => ansi_color("kw-basic"),
                _others => Color::Reset,
            };

            let underline = match (i == focused, modified) {
                (true, true) => Attribute::Underlined,
                (false, true) => Attribute::Underdashed,
                _others => Attribute::NoUnderline,
            };

            let bg_color = SetBackgroundColor(bg_color);
            let fg_color = SetForegroundColor(fg_color);
            let fg_reset = SetForegroundColor(Color::Reset);
            let bg_reset = SetBackgroundColor(default_bg_color());
            let no_line = Attribute::NoUnderline;

            let _ = write!(self.stdout, "  {fg_color}{bg_color}");
            let _ = write!(self.stdout, "{underline}{tab_name}{no_line}");
            let _ = write!(self.stdout, "{fg_reset}{bg_reset}  │");

            queue!(self.stdout, MoveTo(tabs + cursor, 0)).unwrap();
            let _ = write!(self.stdout, "{:─^1$}┬", "", cells);

            queue!(self.stdout, MoveTo(tabs + cursor, 2)).unwrap();
            let _ = write!(self.stdout, "{:─^1$}┴", "", cells);

            cursor += 1 + (cells as u16);
        }

        self.erase_tab_list(cursor);
        let _ = self.stdout.flush();
    }

    pub fn find_tab(&self, x: u16, items: &TabList) -> Option<usize> {
        let mut x = x as usize;

        for (i, (_mod, name)) in items.iter().enumerate() {
            let cells = name.chars().count() + 5;

            match x < cells {
                true => return Some(i),
                false => x -= cells,
            }
        }

        None
    }
}

fn cut_len(text: &str, max: usize) -> (usize, usize) {
    let mut num_chars = 0;
    let mut len_chars = 0;

    for c in text.chars() {
        if num_chars == max {
            break;
        }

        num_chars += 1;
        len_chars += c.len_utf8();
    }

    (len_chars, num_chars)
}

pub fn set_dirty() {
    DIRTY.store(true, Ordering::SeqCst);
}

pub fn restore_term() {
    let mut stdout = stdout();
    let _ = disable_raw_mode();
    queue!(stdout, SetBackgroundColor(Color::Reset)).unwrap();
    queue!(stdout, DisableMouseCapture).unwrap();
    queue!(stdout, LeaveAlternateScreen).unwrap();
    queue!(stdout, RestorePosition).unwrap();
    queue!(stdout, Show).unwrap();
    let _ = stdout.flush();
}
