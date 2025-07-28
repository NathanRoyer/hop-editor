use std::mem::{swap, take};
use std::{io, fs, cmp};
use std::sync::Arc;

use crate::syntax::{Range, SyntaxFile, SyntaxConfig, LineContext};
use crate::interface::TextPart;
use crate::confirm;

const CLOSE_WARNING: &str = "[UNSAVED FILE]\nThis file has unsaved edits!\n- Press Enter to discard the edits.\n- Press Escape to cancel.";

pub type TabList = Vec<(bool, Arc<str>)>;

fn strip_cr<'a>(text: &'a str, eol_cr: &mut bool) -> &'a str {
    let (text, cr) = match text.strip_suffix('\r') {
        Some(text) => (text, true),
        None => (text, false),
    };

    *eol_cr = cr;
    text
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    buffer: String,
    ranges: Vec<Range>,
    eol_ctx: Option<LineContext>,
    eol_cr: bool,
    dirty: bool,
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
}

pub struct DirtyLine<'a> {
    pub line_no: Option<usize>,
    pub tab_width_m1: usize,
    pub text: &'a str,
}

#[derive(Copy, Clone, Default, PartialEq, Eq)]
pub struct Cursor {
    // x is in char unit, not byte
    x: usize,
    y: usize,
    range: usize,
}

pub struct Tab {
    file_path: Option<String>,
    tmp_buf: String,
    name: Arc<str>,

    // state
    lines: Vec<Line>,
    scroll: isize,
    cursors: Vec<Cursor>,
    modified: bool,

    // settings
    tab_lang: Option<String>,
    tab_width_m1: usize,
}

pub struct TabMap {
    inner: Vec<Tab>,
    current: usize,
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

impl Tab {
    fn new(
        file_path: Option<String>,
        text: String,
    ) -> Self {
        let mut line = Line::default();
        line.dirty = true;
        let name = file_name(&file_path);

        let mut this = Self {
            file_path,
            tmp_buf: String::new(),
            name,

            // state
            lines: vec![line],
            scroll: 0,
            cursors: vec![Cursor::default()],
            modified: false,

            // settings
            tab_lang: None,
            tab_width_m1: 3,
        };

        this.insert_text(&text);
        this.modified = false;
        this.tmp_buf = text;

        this.cursors[0] = Cursor::default();

        this
    }

    fn header(&self) -> (bool, Arc<str>) {
        (self.modified, self.name.clone())
    }

    pub fn modified(&self) -> bool {
        self.modified
    }

    fn add_to_cursors(&mut self, first_c: usize, lf: bool, num_x: usize) {
        let len_c = self.cursors.len();
        let y = self.cursors[first_c].y;

        for c in first_c..len_c {
            let cursor = &mut self.cursors[c];

            if lf {
                if cursor.y == y {
                    cursor.x = 0;
                }

                cursor.y += 1;
            } else if y == cursor.y {
                cursor.x += num_x;
            } else {
                break;
            }
        }
    }

    fn insert_text_no_lf(&mut self, c: usize, text: &str) {
        let cursor = &mut self.cursors[c];

        let line = &mut self.lines[cursor.y];
        let offset = line.len_until(cursor.x);
        line.buffer.insert_str(offset, text);
        line.dirty = true;

        self.add_to_cursors(c, false, text.len());
    }

    fn line_feed(&mut self, c: usize, mut eol_cr: bool) {
        let cursor = &self.cursors[c];
        let line = &mut self.lines[cursor.y];

        swap(&mut line.eol_cr, &mut eol_cr);
        let offset = line.len_until(cursor.x);
        let buffer = line.buffer[offset..].into();
        line.buffer.truncate(offset);

        let new_line = Line {
            buffer,
            ranges: vec![],
            eol_ctx: None,
            dirty: true,
            eol_cr,
        };

        let old_y = cursor.y;
        self.lines.insert(old_y + 1, new_line);

        self.add_to_cursors(c, true, 0);
        self.set_lines_dirty(old_y);
    }

    pub fn insert_text(&mut self, text: &str) {
        let cursors = 0..self.cursors.len();

        for c in cursors.rev() {
            let mut iter = text.split('\n');
            let mut eol_cr = false;

            let part = iter.next().unwrap();
            let part = strip_cr(part, &mut eol_cr);
            self.insert_text_no_lf(c, part);

            for part in iter {
                self.line_feed(c, eol_cr);
                let part = strip_cr(part, &mut eol_cr);
                self.insert_text_no_lf(c, part);
            }

            if eol_cr {
                // does the user want this?
                // will someone paste something ending in just \r
                self.insert_text_no_lf(c, "\r");
            }
        }

        self.modified = true;
    }

    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let text = c.encode_utf8(&mut buf);
        self.insert_text(text);
    }

    fn set_lines_dirty(&mut self, from_line: usize) {
        for line in self.lines.iter_mut().skip(from_line) {
            line.dirty = true;
        }
    }

    fn backspace_cursor(&mut self, first_c: usize, merging_lines: bool, num_x: isize) {
        let x_op = |x: usize| x.checked_add_signed(num_x).expect(":(");
        let this_y = self.cursors[first_c].y;
        let iter = self.cursors.iter_mut();

        for cursor in iter.skip(first_c) {
            if cursor.y == this_y {
                cursor.x = x_op(cursor.x);
            } else if !merging_lines {
                break;
            }

            if merging_lines {
                cursor.y -= 1;
            }
        }
    }

    fn merge_with_prev_line(&mut self, c: usize) {
        let this_y = self.cursors[c].y;
        let prev_y = this_y.checked_sub(1).expect("no previous line!");

        let mut line = self.lines.remove(this_y);
        let buf = take(&mut line.buffer);

        let prev = &mut self.lines[prev_y];
        let x_add = prev.len_chars();
        prev.buffer += buf.as_str();

        self.set_lines_dirty(prev_y);
        self.backspace_cursor(c, true, x_add as isize);
    }

    fn backspace(&mut self, c: usize, mut num_chars: usize) {
        while self.cursors[c].x < num_chars {
            self.merge_with_prev_line(c);
            num_chars -= 1;
        }

        // mutable copy! not mut ref
        let cursor = self.cursors[c];
        let line = &mut self.lines[cursor.y];

        let old_i = line.len_until(cursor.x);
        let new_i = line.len_until(cursor.x - num_chars);

        line.buffer.replace_range(new_i..old_i, "");
        line.dirty = true;

        self.backspace_cursor(c, false, -(num_chars as isize));
    }

    pub fn backspace_once(&mut self) {
        for c in 0..self.cursors.len() {
            self.backspace(c, 1);
        }

        self.modified = true;
    }

    pub fn dirty_line<'a>(
        &'a mut self,
        index: u16,
        part_buf: &mut Vec<TextPart>,
        cursors: &mut Vec<usize>,
    ) -> Option<DirtyLine<'a>> {
        let y = self.scroll + (index as isize);
        let tab_width_m1 = self.tab_width_m1;
        part_buf.clear();
        cursors.clear();

        let Ok(y) = usize::try_from(y) else {
            part_buf.push(("wspace", 0));
            return Some(DirtyLine { line_no: None, tab_width_m1, text: "" });
        };

        let Some(line) = self.lines.get_mut(y) else {
            part_buf.push(("wspace", 0));
            return Some(DirtyLine { line_no: None, tab_width_m1, text: "" });
        };

        // return if not dirty
        take(&mut line.dirty).then_some(())?;
        let text = &line.buffer;
        let mut covered = 0;

        // todo: maybe remove this useless copying
        for range in line.ranges.iter() {
            let mode_str = range.mode.as_str();
            part_buf.push((mode_str, range.len));
            covered += range.len;
        }

        if covered < text.len() {
            let missing = text.len() - covered;
            part_buf.push(("wspace", missing));
        }

        let iter_c = self
            .cursors
            .iter()
            .filter(|c| c.y == y)
            .map(|c| c.x);

        cursors.extend(iter_c);
        let line_no = Some(y + 1);

        Some(DirtyLine { line_no, tab_width_m1, text })
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

    fn get_syntax<'a>(&mut self, syntaxes: &'a SyntaxFile) -> Option<&'a SyntaxConfig> {
        if self.tab_lang.is_none() {
            let path = self.file_path.as_ref()?;
            let (_, ext) = path.rsplit_once('.')?;

            self.tab_lang = syntaxes.resolve_ext(ext).map(String::from);
        }

        let lang = self.tab_lang.as_ref()?;

        let Some(syntax) = syntaxes.get(lang) else {
            let valids: Vec<_> = syntaxes.inner.keys().collect();
            confirm!("invalid syntax: {lang:?}\nvalid ones: {valids:?}");
            return None;
        };

        Some(syntax)
    }

    pub fn highlight(&mut self, syntaxes: &SyntaxFile) {
        let Some(syntax) = self.get_syntax(syntaxes) else {
            return;
        };

        let mut ctx = None;

        for line in self.lines.iter_mut() {
            if line.dirty {
                line.eol_ctx = syntax.highlight(ctx, &mut line.ranges, &line.buffer);
            }

            ctx = line.eol_ctx;
        }
    }

    pub fn check_overscroll(&mut self, code_height: u16) {
        let max = self.lines.len().saturating_sub(1) as isize;

        if self.scroll > max {
            self.scroll = max;
        }

        let max = code_height.saturating_sub(1) as isize;

        if self.scroll < -max {
            self.scroll = -max;
        }
    }

    pub fn scroll(&mut self, delta: isize) {
        self.scroll += delta;
        self.set_lines_dirty(0);
    }

    pub fn vertical_jump(&mut self, delta: isize) {
        for c in 0..self.cursors.len() {
            let char_w = |c| match c {
                '\t' => self.tab_width_m1 + 1,
                _ => 1,
            };

            // to-do: check if we're not going
            // back and forth for nothing
            let cursor = self.cursors[c];
            let line = &self.lines[cursor.y];
            let i = line.len_until(cursor.x);
            let x = line.buffer[..i].chars().map(char_w).sum();

            let y = cursor.y as isize + delta;
            if let Ok(y) = usize::try_from(y) {
                if y < self.lines.len() {
                    self.seek_in_line(c, y, x);
                }
            }
        }

        self.check_cursors();
    }

    fn backoff_cursor_once(&mut self, c: usize) {
        let cursor = &mut self.cursors[c];

        if cursor.x != 0 {
            cursor.x -= 1;
        } else if cursor.y != 0 {
            cursor.y -= 1;
            let line = &self.lines[cursor.y];
            cursor.x = line.len_chars();
        }
    }

    fn advance_cursor_once(&mut self, c: usize) {
        let cursor = &mut self.cursors[c];

        let line = &self.lines[cursor.y];
        let lines = self.lines.len();
        let next_x = cursor.x + 1;
        let next_y = cursor.y + 1;

        let has_next_line = next_y < lines;
        let overflow = cursor.x >= line.len_chars();

        if overflow & has_next_line {
            cursor.x = 0;
            cursor.y = next_y;
        } else if !overflow {
            cursor.x = next_x;
        }
    }

    pub fn horizontal_jump(&mut self, delta: isize) {
        type Sig = (usize, fn(&mut Tab, usize));

        let (num_iter, callback): Sig = match delta < 0 {
            true => ((-delta) as usize, Self::backoff_cursor_once),
            false => (delta as usize, Self::advance_cursor_once),
        };

        for c in 0..self.cursors.len() {
            for _ in 0..num_iter {
                let y = self.cursors[c].y;
                self.lines[y].dirty = true;
                callback(self, c);
            }

            let y = self.cursors[c].y;
            self.lines[y].dirty = true;
        }

        self.check_cursors();
    }

    fn seek_in_line(&mut self, c: usize, y: usize, mut x: usize) { // 5
        let cursor = &mut self.cursors[c];
        self.lines[cursor.y].dirty = true;
        let line = &self.lines[y];
        let mut progress = 0;

        for c in line.buffer.chars() {
            if progress >= x {
                break;
            }

            if c == '\t' {
                x = x.saturating_sub(self.tab_width_m1);
            }

            progress += 1;
        }

        cursor.x = progress;
        cursor.y = y;
        self.lines[y].dirty = true;
    }

    pub fn seek(&mut self, x: u16, y: u16, append: bool) {
        let max_y = self.lines.len() as isize;
        let y = y as isize + self.scroll;

        if (y < 0) || (y >= max_y) {
            return;
        }

        if !append {
            for cursor in &self.cursors {
                self.lines[cursor.y].dirty = true;
            }

            self.cursors.clear();
        }

        let c = self.cursors.len();
        self.cursors.push(Cursor::default());
        self.seek_in_line(c, y as usize, x as usize);
        self.check_cursors();
    }

    pub fn check_cursors(&mut self) {
        self.cursors.sort();
        self.cursors.dedup();
    }

    pub fn set_fully_dirty(&mut self) {
        self.set_lines_dirty(0);
    }

    pub fn save(&mut self) {
        self.rebuild();

        let Some(path) = &self.file_path else {
            confirm!("[ERROR]\ncannot save: tab has no underlying file");
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
            inner: vec![Tab::new(None, String::new())],
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

    pub fn open(&mut self, file_path: String) -> Result<(), io::Error> {
        let cur_tab = self.current();
        let replace_current = cur_tab.file_path.is_none() && !cur_tab.modified;

        for (index, tab) in self.inner.iter().enumerate() {
            if tab.file_path.as_ref() == Some(&file_path) {
                self.switch(index);
                return Ok(());
            }
        }

        let tmp_buf = fs::read_to_string(&file_path)?;
        let tab = Tab::new(Some(file_path), tmp_buf);
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

        if self.inner.len() == 0 {
            let tab = Tab::new(None, String::new());
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
