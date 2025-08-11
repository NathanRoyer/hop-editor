use std::mem::{swap, take, replace};
use std::{io, fs, cmp};
use std::fmt::Write;
use std::sync::Arc;

use crate::interface::colored_text::{Part as TextPart, Selection};
use crate::syntax::{Range, SyntaxFile, SyntaxConfig, LineContext};
use crate::{alert, confirm};

use history::History;

mod rendering;
mod clipboard;
mod insertion;
mod deletion;
mod movement;
mod history;

const CLOSE_WARNING: &str = "[UNSAVED FILE]\nReally close This file? It has unsaved edits!";

pub type TabList = Vec<(bool, Arc<str>)>;

#[derive(Clone, Debug, Default)]
struct Line {
    buffer: String,
    ranges: Vec<Range>,
    eol_ctx: Option<LineContext>,
    eol_cr: bool,
    dirty: bool,
}

pub struct DirtyLine<'a> {
    pub horizontal_scroll: usize,
    pub tab_width_m1: usize,
    pub text: &'a str,
}

pub struct Tab {
    file_path: Option<String>,
    tmp_buf: String,
    internal_clipboard: String,
    name: Arc<str>,
    lines: Vec<Line>,
    v_scroll: usize,
    h_scroll: usize,
    cursors: Vec<Cursor>,
    modified: bool,
    syntax: Option<Arc<SyntaxConfig>>,
    tab_width_m1: usize,
    history: History,
}

pub struct TabMap {
    inner: Vec<Tab>,
    current: usize,
}

#[derive(Copy, Clone, PartialEq, Eq, Debug)]
struct Cursor {
    // x is in char unit, not byte
    x: usize,
    y: usize,
    sel_x: isize,
    sel_y: isize,
    id: usize,
}

impl Cursor {
    fn new(id: usize) -> Self {
        Self { x: 0, y: 0, sel_x: 0, sel_y: 0, id }
    }

    fn covers(&self, y: usize) -> bool {
        match self.y.cmp(&y) {
            cmp::Ordering::Greater => self.sel_y < -((self.y - y) as isize),
            cmp::Ordering::Less => self.sel_y > ((y - self.y) as isize),
            cmp::Ordering::Equal => false,
        }
    }

    fn touches(&self, y: usize) -> bool {
        let diff = (y as isize) - (self.y as isize);
        self.sel_y != 0 && diff == self.sel_y
    }

    fn selects(&self) -> bool {
        (self.sel_y != 0) | (self.sel_x != 0)
    }

    fn is_at_sel_end(&self) -> bool {
        match self.sel_y == 0 {
            true => self.sel_x < 0,
            false => self.sel_y < 0,
        }
    }

    fn swap_sel_direction(&mut self) {
        let do_jump = |a: usize, b: isize| {
            a.checked_add_signed(b).unwrap_or(a)
        };

        self.x = do_jump(self.x, self.sel_x);
        self.y = do_jump(self.y, self.sel_y);

        self.sel_x = -self.sel_x;
        self.sel_y = -self.sel_y;
    }

    fn sel_jump(&mut self, to_start: bool) {
        if to_start == self.is_at_sel_end() {
            self.swap_sel_direction();
        }
    }
}

impl Line {
    fn len_chars(&self) -> usize {
        self.buffer.chars().count()
    }

    fn len_until(&self, x: usize) -> usize {
        self
            .buffer
            .chars()
            .take(x)
            .map(|c| c.len_utf8())
            .sum()
    }

    fn half_select(&self, first_half: bool, x_char: usize) -> Selection {
        match first_half {
            true => Selection::new(0, x_char),
            false => Selection::new(x_char, self.len_chars() - x_char),
        }
    }

    fn cells_until(&self, char_x: usize, tab_width_m1: usize) -> usize {
        let char_width = |c| match c {
            '\t' => tab_width_m1 + 1,
            _ => 1,
        };

        let i = self.len_until(char_x);
        self.buffer[..i].chars().map(char_width).sum()
    }
}

impl Tab {
    fn new(
        syntax: Option<Arc<SyntaxConfig>>,
        file_path: Option<String>,
        text: String,
    ) -> Self {
        let name = file_name(&file_path);
        let mut line = Line::default();
        line.dirty = true;

        let mut this = Self {
            file_path,
            tmp_buf: String::new(),
            name,
            lines: vec![line],
            v_scroll: 0,
            h_scroll: 0,
            internal_clipboard: String::new(),
            cursors: vec![Cursor::new(0)],
            modified: false,
            tab_width_m1: 3,
            syntax,
            history: History::new(),
        };

        this.insert_text(&text);
        this.history.activate();
        this.modified = false;
        this.tmp_buf = text;

        this.cursors[0] = Cursor::new(0);

        this
    }

    fn header(&self) -> (bool, Arc<str>) {
        (self.modified, self.name.clone())
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    pub fn has_selections(&self) -> bool {
        self.cursors.iter().any(Cursor::selects)
    }

    pub fn path(&self) -> Option<&str> {
        self.file_path.as_deref()
    }

    fn line_index(&self, screen_y: u16) -> Option<usize> {
        let y = screen_y as usize + self.v_scroll;
        (y < self.lines.len()).then_some(y)
    }

    fn set_lines_dirty(&mut self, from_line: usize) {
        for line in self.lines.iter_mut().skip(from_line) {
            line.dirty = true;
        }
    }

    fn rebuild(&mut self) {
        self.tmp_buf.clear();

        for line in &self.lines {
            self.tmp_buf += &line.buffer;

            if line.eol_cr {
                self.tmp_buf.push('\r');
            }

            self.tmp_buf.push('\n');
        }

        if !self.lines.is_empty() {
            self.tmp_buf.pop();
        }
    }

    pub fn check_cursors(&mut self) {
        self.cursors.sort();
        self.cursors.dedup();
    }

    pub fn cursor_count(&self) -> usize {
        self.cursors.len()
    }

    pub fn cursor_desc(&mut self, c: usize, dst: &mut String) {
        let mut sel_count = 0usize;
        self.extract_selection(c, &mut sel_count);

        let cursor = &self.cursors[c];
        let x = cursor.x + 1;
        let y = cursor.y + 1;
        let _ = write!(dst, " • Line {}, Column {}", y, x);

        if sel_count != 0 {
            let _ = write!(dst, " [{sel_count}]");
        }
    }

    pub fn swap_latest_cursor(&mut self, c: usize) {
        let original = self.latest_cursor();
        let orig_id = self.cursors[original].id;
        let target = &mut self.cursors[c].id;
        let old_id = replace(target, orig_id);
        self.cursors[original].id = old_id;
    }

    pub fn set_fully_dirty(&mut self) {
        self.set_lines_dirty(0);
    }

    pub fn save(&mut self) {
        self.rebuild();

        let Some(path) = &self.file_path else {
            alert!("[ERROR]\ncannot save: tab has no underlying file");
            return;
        };

        if fs::write(path, &self.tmp_buf).is_ok() {
            self.modified = false;
        }
    }
}

impl TabMap {
    pub fn new() -> Self {
        Self {
            inner: vec![Tab::new(None, None, String::new())],
            current: 0,
        }
    }

    pub fn update_tab_list(&self, storage: &mut TabList) -> usize {
        storage.clear();

        for tab in &self.inner {
            storage.push(tab.header());
        }

        self.current
    }

    pub fn current(&mut self) -> &mut Tab {
        &mut self.inner[self.current]
    }

    pub fn open(&mut self, syntaxes: &SyntaxFile, file_path: String) -> Result<(), io::Error> {
        let cur_tab = self.current();
        let replace_current = cur_tab.file_path.is_none() && !cur_tab.modified;

        for (index, tab) in self.inner.iter().enumerate() {
            if tab.file_path.as_ref() == Some(&file_path) {
                self.switch(index);
                return Ok(());
            }
        }

        let mut syntax = None;
        if let Some((_, ext)) = file_path.rsplit_once('.') {
            if let Some(lang) = syntaxes.resolve_ext(ext) {
                syntax = syntaxes.get(lang);
            }
        }

        let tmp_buf = fs::read_to_string(&file_path)?;
        let tab = Tab::new(syntax, Some(file_path), tmp_buf);
        let new_idx = self.inner.len();
        self.inner.push(tab);

        match replace_current {
            true => _ = self.inner.remove(self.current),
            false => self.current = new_idx,
        }

        Ok(())
    }

    pub fn close(&mut self, index: Option<usize>) {
        let index = index.unwrap_or(self.current);

        if self.inner[index].modified && !confirm!("{}", CLOSE_WARNING) {
            return;
        }

        self.inner.remove(index);

        if self.inner.is_empty() {
            let tab = Tab::new(None, None, String::new());
            self.inner.push(tab);
        }

        if self.current == self.inner.len() {
            self.current -= 1;
        }

        self.current().set_fully_dirty();
    }

    pub fn next_tab(&mut self, leftward: bool) {
        let p = self.current + 1;
        let max = self.inner.len().saturating_sub(1);

        let (next, teleport) = match leftward {
            true => ((p <= max).then_some(p), 0),
            false => (self.current.checked_sub(1), max),
        };

        self.current = next.unwrap_or(teleport);
        self.current().set_fully_dirty();
    }

    pub fn switch(&mut self, index: usize) {
        self.current = index;
        self.current().set_fully_dirty();
    }

    pub fn all_saved(&self) -> bool {
        self.inner.iter().all(|t| !t.modified)
    }

    pub fn is_in_use(&self, prefix: &str) -> bool {
        for tab in &self.inner {
            if let Some(path) = tab.path() {
                if path.starts_with(prefix) {
                    return true;
                }
            }
        }

        false
    }
}

impl PartialOrd for Cursor {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Cursor {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match self.y == other.y {
            true => self.x.cmp(&other.x),
            false => self.y.cmp(&other.y),
        }
    }
}

fn strip_cr<'a>(text: &'a str, eol_cr: &mut bool) -> &'a str {
    let (text, cr) = match text.strip_suffix('\r') {
        Some(text) => (text, true),
        None => (text, false),
    };

    *eol_cr = cr;
    text
}

fn file_name(path: &Option<String>) -> Arc<str> {
    let name = match path {
        Some(path) => match path.rsplit_once('/') {
            Some((_, name)) => name,
            None => path,
        },
        None => "[unnamed]",
    };

    name.into()
}
