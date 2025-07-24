#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

use crossterm::{*, terminal::*, event::*, cursor::*, style::*};
use std::io::{stdout, Stdout, Write as _};
use std::fmt::{self, Write as _};
use std::mem::take;

use crate::theme::Theme;

const TREE_WIDTH: u16 = 40;
const TABS_HEIGHT: u16 = 3;
const MENU_HEIGHT: u16 = 6;
const LN_WIDTH: usize = 4;

pub enum UserInput {
    Quit,
    Insert(char),
    Backspace,
    Paste,
    Copy,
    Cut,
    ScrollDown(bool),
    ScrollUp(bool),
    Resize(u16, u16),
    NoOp,
}

pub type ModeName = &'static str;
pub type PartLen = usize;
pub type TextPart = (ModeName, PartLen);

fn cut_len(text: &str, max: usize) -> (Option<usize>, usize) {
    let mut num_chars = 0;
    let mut len_chars = 0;

    for c in text.chars() {
        if num_chars == max {
            return (Some(len_chars), num_chars);
        }

        num_chars += 1;
        len_chars += c.len_utf8();
    }

    (None, num_chars)
}

pub struct ColoredText<'a> {
    parts: &'a [TextPart],
    theme: &'a Theme,
    text: &'a str,
    max: usize,
}

impl<'a> ColoredText<'a> {
    pub fn new(
        parts: &'a [TextPart],
        text: &'a str,
        theme: &'a Theme,
    ) -> Self {
        Self { parts, theme, text, max: 0 }
    }

    fn look_up(&self, name: &str) -> Color {
        type Rgb = (u8, u8, u8);

        match self.theme.get(name) {
            Some(hc) => Color::from(Rgb::from(hc)),
            None => Color::Reset,
        }
    }
}

// todo: rewrite
impl<'a> fmt::Display for ColoredText<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut offset = 0;

        for (mode_name, len) in self.parts {
            let end = offset + len;
            let text = &self.text[offset..end];
            let color = self.look_up(mode_name);
            let tag_a = SetForegroundColor(color);

            if let (Some(cut), _) = cut_len(text, self.max) {
                let tag_b = SetForegroundColor(Color::Reset);
                write!(f, "{tag_a}{}{tag_b}…", &text[..cut])?;
                break;
            } else {
                write!(f, "{tag_a}{text}")?;
            }

            offset = end;
        }

        Ok(())
    }
}

pub struct Interface {
    str_buf: String,
    stdout: Stdout,
    height: u16,
    width: u16,
}

impl Interface {
    pub fn new() -> Self {
        let mut stdout = stdout();
        queue!(stdout, SavePosition).unwrap();
        queue!(stdout, EnterAlternateScreen).unwrap();
        queue!(stdout, EnableMouseCapture).unwrap();
        queue!(stdout, Hide).unwrap();
        let _ = enable_raw_mode();
        let _ = stdout.flush();

        let (width, height) = size().unwrap();

        Self {
            str_buf: String::with_capacity(1024),
            stdout,
            height,
            width,
        }
    }

    pub fn close(mut self) {
        let _ = disable_raw_mode();
        queue!(self.stdout, DisableMouseCapture).unwrap();
        queue!(self.stdout, LeaveAlternateScreen).unwrap();
        queue!(self.stdout, RestorePosition).unwrap();
        queue!(self.stdout, Show).unwrap();
        let _ = self.stdout.flush();
    }

    pub fn code_height(&self) -> u16 {
        self.height.saturating_sub(TABS_HEIGHT)
    }

    fn code_width(&self) -> usize {
        self.width.saturating_sub(TREE_WIDTH + 1).into()
    }

    fn erase_tab_list(&mut self, offset: u16) {
        let x = TREE_WIDTH + 1 + offset;
        let len = self.code_width().saturating_sub(offset.into());

        queue!(self.stdout, MoveTo(x, 0)).unwrap();
        let _ = write!(self.stdout, "{:╌^1$}", "", len);

        queue!(self.stdout, MoveTo(x, 1)).unwrap();
        let _ = write!(self.stdout, "{: ^1$}", "", len);

        queue!(self.stdout, MoveTo(x, 2)).unwrap();
        let _ = write!(self.stdout, "{:╌^1$}", "", len);
    }

    pub fn draw_decorations(&mut self) {
        queue!(self.stdout, MoveTo(0, MENU_HEIGHT)).unwrap();
        let _ = write!(self.stdout, "{:─^1$}", " Folders ", TREE_WIDTH as _);

        // queue!(self.stdout, SetForegroundColor(Color::DarkGrey)).unwrap();

        for y in 0..self.height {
            queue!(self.stdout, MoveTo(TREE_WIDTH, y)).unwrap();

            let c = match y {
                0 | 2 => '├',
                MENU_HEIGHT => '┤',
                _ => '│',
            };

            let _ = write!(self.stdout, "{}", c);
        }

        self.erase_tab_list(0);

        // queue!(self.stdout, ResetColor).unwrap();
        let _ = self.stdout.flush();
    }

    pub fn write_text<T: fmt::Display>(&mut self, x: u16, y: u16, text: T) {
        queue!(self.stdout, SetForegroundColor(Color::Reset)).unwrap();
        queue!(self.stdout, MoveTo(x, y)).unwrap();
        let _ = write!(self.stdout, "{}", &text);
        let _ = self.stdout.flush();
    }

    pub fn set_tree_row(&mut self, index: u16, text: &str) {
        let max = TREE_WIDTH.min(self.width) as usize;
        let (cut, chars) = cut_len(text, max);
        let cut = cut.unwrap_or(text.len());
        self.write_text(0, MENU_HEIGHT + 1 + index, &text[..cut]);
        let _ = write!(self.stdout, "{:1$}", "", max - chars);

        // "▶  ▷ ▼     ▽"

        let _ = self.stdout.flush();
    }

    pub fn set_code_row(&mut self, index: u16, line_no: usize, mut text: ColoredText) {
        let mut buf = take(&mut self.str_buf);
        buf.clear();
        let _ = write!(&mut buf, "{:1$} ", line_no, LN_WIDTH);

        let y = TABS_HEIGHT + index;
        let mut x = TREE_WIDTH + 1;
        self.write_text(x, y, &buf);

        x += LN_WIDTH as u16 + 2;
        text.max = self.width.saturating_sub(x) as usize;
        self.write_text(x, y, text);
        let _ = queue!(self.stdout, Clear(ClearType::UntilNewLine));

        self.str_buf = buf;
        let _ = self.stdout.flush();
    }

    pub fn set_tab_list(&mut self, items: &[&str]) {
        let tabs = TREE_WIDTH + 1;
        let mut cursor = 0;

        for tab_name in items {
            queue!(self.stdout, MoveTo(tabs + cursor, 1)).unwrap();
            let _ = write!(self.stdout, " {tab_name} │");

            let len = tab_name.len() + 2;

            queue!(self.stdout, MoveTo(tabs + cursor, 0)).unwrap();
            let _ = write!(self.stdout, "{:╌^1$}┬", "", len);

            queue!(self.stdout, MoveTo(tabs + cursor, 2)).unwrap();
            let _ = write!(self.stdout, "{:╌^1$}┴", "", len);

            cursor += 1 + (len as u16);
        }

        self.erase_tab_list(cursor);
        let _ = self.stdout.flush();
    }

    pub fn set_toolbar(&self, items: &[&str]) {
        todo!()
    }

    pub fn set_status(&self, status: &str) {
        todo!()
    }

    pub fn read_event(&self) -> UserInput {
        match read().unwrap() {
            Event::Key(e) if e.code == KeyCode::Esc => UserInput::Quit,
            Event::Key(KeyEvent { code: KeyCode::Char(c), .. }) => UserInput::Insert(c),
            Event::Key(KeyEvent { code: KeyCode::Enter, .. }) => UserInput::Insert('\n'),
            Event::Key(KeyEvent { code: KeyCode::Backspace, .. }) => UserInput::Backspace,
            Event::Key(KeyEvent { code: KeyCode::PageDown, .. }) => UserInput::ScrollDown(true),
            Event::Key(KeyEvent { code: KeyCode::PageUp, .. }) => UserInput::ScrollUp(true),
            Event::Mouse(e) if e.kind == MouseEventKind::ScrollDown => UserInput::ScrollDown(false),
            Event::Mouse(e) if e.kind == MouseEventKind::ScrollUp => UserInput::ScrollUp(false),
            Event::Resize(w, h) => UserInput::Resize(w, h),
            _other => UserInput::NoOp,
        }
    }
}
