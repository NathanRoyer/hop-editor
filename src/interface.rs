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
    CodeDrag(u16, u16),
    ClearDrag,
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

#[derive(Debug)]
enum Location {
    PanelSep,
    Menu,
    MenuEdge,
    TreeRow(u16),
    Tab(u16),
    LineNo(u16),
    Code(u16, u16),
}

pub type ModeName = &'static str;
pub type PartLen = usize;
pub type TextPart = (ModeName, PartLen);

// todo get rid
fn cut_len(text: &str, max: usize) -> (Option<usize>, usize) {
    let mut num_chars = 0;
    let mut len_chars = 0;

    for c in text.chars() {
        let next = num_chars + match c {
            // todo use var
            '\t' => 4,
            _ => 1,
        };

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
    tab_width_m1: usize,
    max: usize,
}

impl<'a> ColoredText<'a> {
    pub fn new(
        parts: &'a [TextPart],
        cursors: &'a [usize],
        text: &'a str,
        tab_width_m1: usize,
        theme: &'a Theme,
    ) -> Self {
        Self { parts, theme, text, cursors, tab_width_m1, max: 0 }
    }
}

fn write_rev(f: &mut fmt::Formatter, c: char) -> Result<(), fmt::Error> {
    let rev1 = SetAttribute(Attribute::Reverse);
    let rev2 = SetAttribute(Attribute::NoReverse);
    write!(f, "{rev1}{c}{rev2}")
}

fn write_text_cursor(
    f: &mut fmt::Formatter,
    // sorted
    cursors: &[usize],
    color: Color,
    text: &str,
    num_taken: &mut usize,
    num_visible: &mut usize,
    tab_width_m1: usize,
    max: usize,
) -> Result<bool, fmt::Error> {
    write!(f, "{}", SetForegroundColor(color))?;
    let mut cursors = cursors.iter();
    let mut cursor = cursors.next();

    for c in text.chars() {
        let mut c_disp = c;
        let mut addition = 1;

        if c == '\t' {
            addition += tab_width_m1;
            c_disp = ' ';
        }

        if *num_visible + addition >= max {
            return Ok(true);
        }

        if Some(*num_taken) == cursor.copied() {
            cursor = cursors.next();
            write_rev(f, c_disp)?;
        } else {
            write!(f, "{c_disp}")?;
        }

        if c == '\t' {
            let _ = write!(f, "{:^1$}", "", tab_width_m1);
        }

        *num_visible += addition;
        *num_taken += 1;
    }

    Ok(false)
}

impl<'a> fmt::Display for ColoredText<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        let mut overflow = false;
        let mut num_visible = 0;
        let mut num_taken = 0;
        let mut offset = 0;

        for (mode_name, len) in self.parts {
            let end = offset + len;
            let text = &self.text[offset..end];
            let color = self.theme.get_ansi(mode_name);

            overflow = write_text_cursor(
                f,
                self.cursors,
                color,
                text,
                &mut num_taken,
                &mut num_visible,
                self.tab_width_m1,
                self.max,
            )?;

            if overflow {
                write!(f, "{}…", SetForegroundColor(Color::Reset))?;
                break;
            }

            offset = end;
        }

        if self.cursors.contains(&offset) {
            if overflow {
                write!(f, "{}", MoveLeft(1))?;
            }

            write_rev(f, ' ')?;
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

    pub fn write_text<T: fmt::Display>(&mut self, x: u16, y: u16, text: T) {
        queue!(self.stdout, SetForegroundColor(Color::Reset)).unwrap();
        queue!(self.stdout, MoveTo(x, y)).unwrap();
        let _ = write!(self.stdout, "{text}");
        let _ = self.stdout.flush();
    }

    pub fn set_tree_row(
        &mut self,
        hovered: Option<u16>,
        index: u16,
        text: &str,
        theme: &Theme,
    ) {
        if hovered == Some(index) {
            let color = theme.get_ansi("hover-bg");
            queue!(self.stdout, SetBackgroundColor(color)).unwrap();
        }
        let max = TREE_WIDTH.min(self.width) as usize;
        let (cut, chars) = cut_len(text, max);
        let cut = cut.unwrap_or(text.len());
        self.write_text(0, MENU_HEIGHT + 1 + index, &text[..cut]);
        queue!(self.stdout, SetBackgroundColor(Color::Reset)).unwrap();
        let _ = write!(self.stdout, "{:1$}", "", max - chars);

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
        self.write_text(x, y, &buf);

        x += LN_WIDTH as u16 + 2;
        text.max = self.width.saturating_sub(x) as usize;
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
            let _ = write!(self.stdout, "{:─^1$}┬", "", len);

            queue!(self.stdout, MoveTo(tabs + cursor, 2)).unwrap();
            let _ = write!(self.stdout, "{:─^1$}┴", "", len);

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

    fn cursor_pos(&self, x: u16, y: u16) -> Location {
        let code_x = TREE_WIDTH + (LN_WIDTH as u16) + 3;
        let tree_y = MENU_HEIGHT + 1;

        if x == TREE_WIDTH {
            Location::PanelSep
        } else if x < TREE_WIDTH {
            if y < MENU_HEIGHT {
                Location::Menu
            } else if y >= tree_y {
                Location::TreeRow(y - tree_y)
            } else {
                Location::MenuEdge
            }
        } else if y < 3 {
            Location::Tab(x - TREE_WIDTH - 1)
        } else if x < code_x {
            Location::LineNo(y - 3)
        } else {
            Location::Code(x - code_x, y - 3)
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
                use {MouseEventKind::*, MouseButton::*};
                let ctrl = e.modifiers.contains(KeyModifiers::CONTROL);
                let pos = self.cursor_pos(e.column, e.row);
                let debug = || {
                    confirm!("invalid action:\n- event: {e:?}\n- pos: {pos:?}");
                    UserInput::NoOp
                };

                match pos {
                    Location::Code(x, y) => match e.kind {
                        MouseEventKind::ScrollDown => UserInput::Scroll(1),
                        MouseEventKind::ScrollUp => UserInput::Scroll(-1),
                        Down(Left) => UserInput::CodeSeek(x, y, ctrl),
                        Drag(Left) => UserInput::CodeDrag(x, y),
                        MouseEventKind::Up(_) => UserInput::ClearDrag,
                        Moved => UserInput::ClearHover,
                        _ => debug(),
                    },
                    Location::TreeRow(y) => match e.kind {
                        MouseEventKind::ScrollDown => UserInput::Scroll(1),
                        MouseEventKind::ScrollUp => UserInput::Scroll(-1),
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        Down(Left) => UserInput::TreeClick(y),
                        Moved => UserInput::TreeHover(y),
                        _ => debug(),
                    },
                    Location::Tab(x) => match e.kind {
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        Down(Left) => UserInput::TabClick(x),
                        Moved => UserInput::TabHover(x),
                        _ => debug(),
                    },
                    Location::LineNo(y) => match e.kind {
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        _ => debug(),
                    },
                    Location::PanelSep => match e.kind {
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        _ => debug(),
                    },
                    Location::MenuEdge => match e.kind {
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        _ => debug(),
                    },
                    Location::Menu => match e.kind {
                        MouseEventKind::Up(_) => UserInput::NoOp,
                        Moved => UserInput::ClearHover,
                        _ => debug(),
                    },
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
