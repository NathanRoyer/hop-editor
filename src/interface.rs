#![allow(unused_variables)]
#![allow(unused_imports)]
#![allow(dead_code)]

use crossterm::{*, terminal::*, event::*, cursor::*, style::*};
use std::io::{stdout, Stdout, Write as _};
use std::sync::atomic::{AtomicBool, Ordering};
use std::fmt::{self, Write as _};
use std::mem::take;

use crate::theme::Theme;
use crate::tab::TabList;
use crate::confirm;

const TREE_WIDTH: u16 = 40;
const TABS_HEIGHT: u16 = 3;
const MENU_HEIGHT: u16 = 4;
const LN_WIDTH: usize = 4;

static DIRTY: AtomicBool = AtomicBool::new(true);

#[derive(Copy, Clone)]
pub enum UserInput {
    Quit,
    Save,
    CloseTab,
    NextTab(bool),
    Insert(char),
    CodeSeek(u16, u16, bool),
    TreeClick(u16),
    TreeHover(u16),
    TabHover(u16),
    ClearHover,
    TabClick(u16),
    Backspace,
    Paste,
    Copy,
    Cut,
    Scroll(isize),
    Resize(u16, u16),
    HorizontalJump(isize),
    VerticalJump(isize),
    NoOp,
}

pub type ModeName = &'static str;
pub type PartLen = usize;
pub type TextPart = (ModeName, PartLen);

fn cut_len(text: &str, max: usize) -> (Option<usize>, usize) {
    let mut num_chars = 0;
    let mut len_chars = 0;

    for c in text.chars() {
        let next = num_chars + 1;

        if next == max {
            return (Some(len_chars), num_chars);
        }

        num_chars = next;
        len_chars += c.len_utf8();
    }

    (None, num_chars)
}

pub struct ColoredText<'a> {
    parts: &'a [TextPart],
    cursors: &'a [usize],
    theme: &'a Theme,
    text: &'a str,
    max: usize,
}

impl<'a> ColoredText<'a> {
    pub fn new(
        parts: &'a [TextPart],
        cursors: &'a [usize],
        text: &'a str,
        theme: &'a Theme,
    ) -> Self {
        Self { parts, theme, text, cursors, max: 0 }
    }
}

fn write_text_cursor(
    f: &mut fmt::Formatter,
    // sorted
    cursors: &[usize],
    color: Color,
    mut offset: usize,
    mut text: &str,
) -> Result<(), fmt::Error> {
    let tag_a = SetForegroundColor(color);

    for cursor in cursors {
        if let Some(prefix_len) = cursor.checked_sub(offset) {
            if prefix_len < text.len() {
                let (prefix_str, rest) = text.split_at(prefix_len);
                let middle_char = rest.chars().next().unwrap();
                let charlene = middle_char.len_utf8();
                offset += prefix_len + charlene;
                text = &rest[charlene..];

                let tag_b = SetAttribute(Attribute::Reverse);
                let tag_c = SetAttribute(Attribute::NoReverse);

                write!(f, "{tag_a}{prefix_str}")?;
                write!(f, "{tag_b}{middle_char}{tag_c}")?;
            }
        }
    }

    write!(f, "{tag_a}{text}")
}

impl<'a> fmt::Display for ColoredText<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut overflow = false;
        let mut max = self.max;
        let mut offset = 0;

        for (mode_name, len) in self.parts {
            let end = offset + len;
            let text = &self.text[offset..end];
            let color = self.theme.get_ansi(mode_name);

            if let (Some(cut), _) = cut_len(text, max) {
                let text = &text[..cut];
                let tag_b = SetForegroundColor(Color::Reset);
                write_text_cursor(f, self.cursors, color, offset, text)?;
                write!(f, "{tag_b}…")?;
                overflow = true;
                break;
            } else {
                write_text_cursor(f, self.cursors, color, offset, text)?;
            }

            offset = end;
            max = max.saturating_sub(*len);
        }

        if self.cursors.contains(&offset) {
            if overflow {
                write!(f, "{}", MoveLeft(1))?;
            }

            let tag_b = SetAttribute(Attribute::Reverse);
            let tag_c = SetAttribute(Attribute::NoReverse);
            write!(f, "{tag_b} {tag_c}")?;
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

    pub fn code_height(&self) -> u16 {
        self.height.saturating_sub(TABS_HEIGHT)
    }

    pub fn tree_height(&self) -> u16 {
        self.height.saturating_sub(MENU_HEIGHT + 1)
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
        queue!(self.stdout, Clear(ClearType::All)).unwrap();
        queue!(self.stdout, SetForegroundColor(Color::Reset)).unwrap();
        queue!(self.stdout, MoveTo(0, MENU_HEIGHT)).unwrap();
        let _ = write!(self.stdout, "{:─^1$}", " Folders ", TREE_WIDTH as _);

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

        let _ = self.stdout.flush();
    }

    pub fn write_text<T: fmt::Display>(&mut self, x: u16, y: u16, indent: usize, text: T) {
        queue!(self.stdout, SetForegroundColor(Color::Reset)).unwrap();
        queue!(self.stdout, MoveTo(x, y)).unwrap();
        let _ = write!(self.stdout, "{:1$}{text}", "", indent);
        let _ = self.stdout.flush();
    }

    pub fn set_tree_row(
        &mut self,
        hovered: Option<u16>,
        index: u16,
        indent: usize,
        text: &str,
        theme: &Theme,
    ) {
        if hovered == Some(index) {
            let color = theme.get_ansi("hover-bg");
            queue!(self.stdout, SetBackgroundColor(color)).unwrap();
        }
        let max = TREE_WIDTH.min(self.width) as usize - indent;
        let (cut, chars) = cut_len(text, max);
        let cut = cut.unwrap_or(text.len());
        self.write_text(0, MENU_HEIGHT + 1 + index, indent, &text[..cut]);
        queue!(self.stdout, SetBackgroundColor(Color::Reset)).unwrap();
        let _ = write!(self.stdout, "{:1$}", "", max - chars);

        // "▶  ▷ ▼     ▽"

        let _ = self.stdout.flush();
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
        let mut x = TREE_WIDTH + 1;
        self.write_text(x, y, 0, &buf);

        x += LN_WIDTH as u16 + 2;
        text.max = self.width.saturating_sub(x) as usize;
        self.write_text(x, y, 0, text);
        let _ = queue!(self.stdout, Clear(ClearType::UntilNewLine));

        self.str_buf = buf;
        let _ = self.stdout.flush();
    }

    pub fn set_tab_list(
        &mut self,
        hover_pos: Option<u16>,
        focused: usize,
        items: &TabList,
        theme: &Theme,
    ) {
        let tabs = TREE_WIDTH + 1;
        let mut cursor = 0;

        for (i, (modified, tab_name)) in items.iter().enumerate() {
            queue!(self.stdout, MoveTo(tabs + cursor, 1)).unwrap();
            let len = tab_name.len() + 2;
            let mut hovered = false;

            if let Some(pos) = hover_pos {
                hovered = match pos.checked_sub(cursor) {
                    Some(rem) => (rem as usize) < len,
                    None => false,
                };
            }

            let bg_color = match hovered {
                true => theme.get_ansi("hover-bg"),
                false => Color::Reset,
            };

            let fg_color = match (i == focused, modified) {
                (true, false) => theme.get_ansi("kw-strong"),
                (true, true) => theme.get_ansi("kw-basic"),
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
            let bg_reset = SetBackgroundColor(Color::Reset);
            let no_line = Attribute::NoUnderline;

            let _ = write!(self.stdout, " {fg_color}{bg_color}");
            let _ = write!(self.stdout, "{underline}{tab_name}{no_line}");
            let _ = write!(self.stdout, "{fg_reset}{bg_reset} │");

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

    pub fn find_tab(&self, x: u16, items: &TabList) -> Option<usize> {
        let mut x = x as usize;

        for (i, (_mod, name)) in items.iter().enumerate() {
            let len = name.len() + 3;

            match x < len {
                true => return Some(i),
                false => x -= len,
            }
        }

        None
    }

    fn on_mouse_event(&self, mut x: u16, mut y: u16, ctrl: bool) -> [UserInput; 2] {
        let code_x = TREE_WIDTH + (LN_WIDTH as u16) + 3;
        let tree_y = MENU_HEIGHT + 1;

        if x == TREE_WIDTH {
            [UserInput::NoOp, UserInput::ClearHover]
        } else if x < TREE_WIDTH {
            // left panel
            if y < MENU_HEIGHT {
                // menu
                [UserInput::NoOp, UserInput::ClearHover]
            } else if y >= tree_y {
                // tree
                y -= tree_y;
                [UserInput::TreeClick(y), UserInput::TreeHover(y)]
            } else {
                // on menu edge
                [UserInput::NoOp, UserInput::ClearHover]
            }
        } else if y < 3 {
            // tabs
            x = x - TREE_WIDTH - 1;
            [UserInput::TabClick(x), UserInput::TabHover(x)]
        } else if x < code_x {
            // line numbers
            [UserInput::NoOp, UserInput::ClearHover]
        } else {
            // in code
            [UserInput::CodeSeek(x - code_x, y - 3, ctrl), UserInput::ClearHover]
        }
    }

    pub fn read_event(&self) -> UserInput {
        let code_height = self.code_height() as isize;

        match read().unwrap() {
            Event::Key(e) if !e.is_release() => {
                let shift = e.modifiers.contains(KeyModifiers::SHIFT);

                if e.modifiers.contains(KeyModifiers::CONTROL) {
                    match e.code {
                        KeyCode::Right => UserInput::HorizontalJump(10),
                        KeyCode::Left => UserInput::HorizontalJump(-10),
                        KeyCode::Char('w') => UserInput::CloseTab,
                        KeyCode::Char('q') => UserInput::Quit,
                        KeyCode::Char('s') => UserInput::Save,
                        KeyCode::Down => UserInput::Scroll(1),
                        KeyCode::Up => UserInput::Scroll(-1),
                        _ => (confirm!("unk-ev: {e:?}"), UserInput::NoOp).1,
                    }
                } else {
                    match e.code {
                        KeyCode::PageDown if shift => UserInput::NextTab(true),
                        KeyCode::PageUp if shift => UserInput::NextTab(false),
                        KeyCode::PageDown => UserInput::Scroll(code_height),
                        KeyCode::PageUp => UserInput::Scroll(-code_height),
                        KeyCode::Right => UserInput::HorizontalJump(1),
                        KeyCode::Left => UserInput::HorizontalJump(-1),
                        KeyCode::Down => UserInput::VerticalJump(1),
                        KeyCode::Up => UserInput::VerticalJump(-1),
                        KeyCode::Backspace => UserInput::Backspace,
                        KeyCode::Char(c) => UserInput::Insert(c),
                        KeyCode::Enter => UserInput::Insert('\n'),
                        KeyCode::Tab => UserInput::Insert('\t'),
                        KeyCode::Esc => UserInput::Quit,
                        _ => (confirm!("unk-ev: {e:?}"), UserInput::NoOp).1,
                    }
                }
            },
            Event::Mouse(e) => {
                let ctrl = e.modifiers.contains(KeyModifiers::CONTROL);
                let events = self.on_mouse_event(e.column, e.row, ctrl);

                if ctrl {
                    match e.kind {
                        MouseEventKind::Down(MouseButton::Left) => events[0],
                        MouseEventKind::Moved => events[1],
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        _ => (confirm!("unk-ev: {e:?}"), UserInput::NoOp).1,
                    }
                } else {
                    match e.kind {
                        MouseEventKind::ScrollDown => UserInput::Scroll(1),
                        MouseEventKind::ScrollUp => UserInput::Scroll(-1),
                        MouseEventKind::Down(MouseButton::Left) => events[0],
                        MouseEventKind::Moved => events[1],
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        _ => (confirm!("unk-ev: {e:?}"), UserInput::NoOp).1,
                    }
                }
            },
            Event::Resize(w, h) => UserInput::Resize(w, h),
            e => (confirm!("unk-ev: {e:?}"), UserInput::NoOp).1,
        }
    }
}

pub fn set_dirty() {
    DIRTY.store(true, Ordering::SeqCst);
}

#[macro_export]
macro_rules! confirm {
    ($entry:expr $(, $arg:expr)* $(,)?) => {
        crate::interface::_confirm(format!($entry, $($arg),*))
    }
}

pub fn _confirm(text: String) -> bool {
    set_dirty();

    let mut stdout = stdout();
    queue!(stdout, Clear(ClearType::All)).unwrap();
    queue!(stdout, MoveTo(8, 4)).unwrap();
    write!(stdout, "{:╌^1$}", "", 40).unwrap();

    for (i, line) in text.split('\n').enumerate() {
        let y = i as u16 + 5;
        queue!(stdout, MoveTo(8, y)).unwrap();
        write!(stdout, "{line}").unwrap();
    }

    let _ = stdout.flush();

    loop {
        match read().unwrap() {
            Event::Key(e) if !e.is_release() => match e.code {
                KeyCode::Enter => break true,
                KeyCode::Esc => break false,
                _other => (),
            },
            _other => (),
        }
    }
}

pub fn restore_term() {
    let mut stdout = stdout();
    let _ = disable_raw_mode();
    queue!(stdout, DisableMouseCapture).unwrap();
    queue!(stdout, LeaveAlternateScreen).unwrap();
    queue!(stdout, RestorePosition).unwrap();
    queue!(stdout, Show).unwrap();
    let _ = stdout.flush();
}
