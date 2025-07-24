use std::sync::atomic::{AtomicU64, Ordering};
use std::mem::{swap, take};
use std::{io, fs};

use crate::syntax::{Range, SyntaxFile};
use crate::interface::TextPart;

use litemap::LiteMap;

fn strip_cr<'a>(text: &'a str, eol_cr: &mut bool) -> &'a str {
    let (text, cr) = match text.strip_suffix('\r') {
        Some(text) => (text, true),
        None => (text, false),
    };

    *eol_cr = cr;
    text
}

static NEXT_KEY: AtomicU64 = AtomicU64::new(0);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TabKey {
    inner: u64,
}

impl TabKey {
    pub fn new() -> Self {
        Self { inner: NEXT_KEY.fetch_add(1, Ordering::SeqCst) }
    }
}

#[derive(Clone, Debug, Default)]
pub struct Line {
    buffer: String,
    // did the line end with \r
    eol_cr: bool,
    dirty: bool,
}

pub struct DirtyLine<'a> {
    pub line_no: usize,
    pub text: &'a str,
}

pub struct Tab {
    file_path: Option<String>,
    tmp_buf: String,

    // state
    ranges: Vec<Range>,
    lines: Vec<Line>,
    scroll: isize,
    target_x: Option<usize>,
    cursor_x: usize,
    cursor_y: usize,
    cursor_r: usize,

    // settings
    tab_lang: String,
    tab_width: usize,
}

pub struct TabMap {
    inner: LiteMap<TabKey, Tab>,
    current: TabKey,
}

impl Line {
    pub fn len_bytes(&self) -> usize {
        self.buffer.len() + (self.eol_cr as usize)
    }

    pub fn len_chars(&self) -> usize {
        self.buffer.chars().count()
    }
}

impl Tab {
    fn new(
        file_path: Option<String>,
        text: String,
    ) -> Self {
        let mut range = Range::default();
        let mut line = Line::default();
        range.len = text.len();
        line.dirty = true;

        let mut this = Self {
            file_path,
            tmp_buf: String::new(),

            // state
            ranges: vec![range],
            lines: vec![line],
            scroll: 0,
            target_x: None,
            cursor_x: 0,
            cursor_y: 0,
            cursor_r: 0,

            // settings
            tab_lang: String::from("rust"),
            tab_width: 4,
        };

        this.insert_text(&text);
        this.tmp_buf = text;

        // this.cursor_x = 0;
        // this.cursor_y = 0;
        // this.cursor_r = 0;

        this
    }

    fn name(&self) -> &str {
        match &self.file_path {
            Some(path) => match path.rsplit_once('/') {
                Some((_, name)) => name,
                None => path,
            },
            None => "[unnamed]",
        }
    }

    fn insert_text_no_lf(&mut self, text: &str) {
        self.ranges[self.cursor_r].len += text.len();

        let line = &mut self.lines[self.cursor_y];
        line.buffer.insert_str(self.cursor_x, text);
        line.dirty = true;

        self.cursor_x += text.len();
        self.target_x.take();
    }

    fn line_feed(&mut self, mut eol_cr: bool) {
        let range = &mut self.ranges[self.cursor_r];
        range.len += 1 + (eol_cr as usize);

        let line = &mut self.lines[self.cursor_y];
        let buffer = line.buffer[self.cursor_x..].into();
        swap(&mut line.eol_cr, &mut eol_cr);
        line.dirty = true;

        let new_line = Line {
            buffer,
            dirty: true,
            eol_cr,
        };

        line.buffer.truncate(self.cursor_x);
        self.cursor_y += 1;
        self.cursor_x = 0;
        self.target_x.take();
        self.lines.insert(self.cursor_y, new_line);
    }

    pub fn insert_text(&mut self, text: &str) {
        let mut iter = text.split('\n');
        let mut eol_cr = false;

        let part = iter.next().unwrap();
        let part = strip_cr(part, &mut eol_cr);
        self.insert_text_no_lf(part);

        for part in iter {
            self.line_feed(eol_cr);
            let part = strip_cr(part, &mut eol_cr);
            self.insert_text_no_lf(part);
        }

        if eol_cr {
            // does the user want this?
            // will someone paste something ending in just \r
            self.insert_text_no_lf("\r");
        }
    }

    fn merge_with_prev_line(&mut self) -> usize {
        let prev_y = self
            .cursor_y
            .checked_sub(1)
            .expect("no previous line!");

        let mut line = self.lines.remove(self.cursor_y);
        let buf = take(&mut line.buffer);

        let prev = &mut self.lines[prev_y];
        self.cursor_x += prev.buffer.len();
        prev.buffer += buf.as_str();

        self.cursor_y = prev_y;
        1 + (prev.eol_cr as usize)

    }

    pub fn backspace(&mut self, mut bytes: usize) {
        while self.cursor_x < bytes {
            bytes -= self.merge_with_prev_line();
        }

        let line = &mut self.lines[self.cursor_y];
        let new_cursor = self.cursor_x - bytes;
        let range = new_cursor..self.cursor_x;
        line.buffer.replace_range(range, "");
    }

    pub fn dirty_line<'a>(
        &'a mut self,
        index: usize,
        part_buf: &mut Vec<TextPart>,
    ) -> Option<DirtyLine<'a>> {
        let y = self.scroll + (index as isize);
        part_buf.clear();

        let Ok(y) = usize::try_from(y) else {
            part_buf.push(("wspace", 0));
            return Some(DirtyLine { line_no: 0, text: "" });
        };

        let Some(line) = self.lines.get_mut(y) else {
            part_buf.push(("wspace", 0));
            return Some(DirtyLine { line_no: 0, text: "" });
        };

        let dirty = take(&mut line.dirty);

        if dirty {
            let (r, mut o) = self.range_at_line(y);

            let text = &self.lines[y].buffer;
            let mut remaining = text.len();

            for range in self.ranges.iter().skip(r) {
                let mode_str = range.mode.as_str();
                let len = (range.len - o).min(remaining);
                part_buf.push((mode_str, len));
                remaining -= len;
                o = 0;

                if remaining == 0 {
                    break;
                }
            }

            Some(DirtyLine { line_no: y + 1, text })
        } else {
            None
        }
    }

    fn range_at_line(&self, index: usize) -> (usize, usize) {
        let mut iter = self.ranges.iter();
        let mut range = *iter.next().expect("should not happen");
        let mut offset = 0;
        let mut i = 0;

        // for each line before this one
        for line in self.lines.iter().take(index) {
            let mut len_crlf = line.len_bytes() + 1;
            range.len -= offset;

            while len_crlf >= range.len {
                i += 1;
                len_crlf -= range.len;

                match iter.next() {
                    Some(r) => range = *r,
                    None => break,
                }
            }

            offset = len_crlf;
        }

        (i, offset)
    }

    // returns cursor offset in tmp_buf
    fn rebuild(&mut self) -> usize {
        self.tmp_buf.clear();
        let mut cursor_o = 0;

        for (y, line) in self.lines.iter().enumerate() {
            self.tmp_buf += &line.buffer;

            if y == self.cursor_y {
                cursor_o = self.tmp_buf.len() + self.cursor_x;
            }

            if line.eol_cr {
                self.tmp_buf.push('\r');
            }

            self.tmp_buf.push('\n');
        }

        if !self.lines.is_empty() {
            self.tmp_buf.pop();
        }

        cursor_o
    }

    pub fn error(&mut self, message: String) {
        *self = Tab::new(None, message);
    }

    pub fn highlight(&mut self, syntaxes: &SyntaxFile) {
        let Some(syntax) = syntaxes.get(&self.tab_lang) else {
            let valids: Vec<_> = syntaxes.inner.keys().collect();
            self.error(format!("valid syntaxes: {valids:?}"));
            return;
        };

        let mut cursor_o = self.rebuild();
        self.ranges = syntax.highlight(&self.tmp_buf);

        for (i, range) in self.ranges.iter().enumerate() {
            if cursor_o < range.len {
                self.cursor_r = i;
                break;
            } else {
                cursor_o -= range.len;
            }
        }

        let mut dump = String::new();
        let mut start = 0;
        for range in &self.ranges.clone() {
            let stop = start + range.len;
            dump += &format!("{} {}: {}\n", range.mode.as_str(), range.len, &self.tmp_buf[start..stop]);
            start = stop;
        }

        fs::write("dump.txt", dump).unwrap();
    }

    pub fn scroll(&mut self, delta: isize) {
        self.scroll += delta;
        self.lines.iter_mut().for_each(|l| l.dirty = true);
    }
}

impl TabMap {
    pub fn new() -> Self {
        let current = TabKey::new();
        let tabs = [(current, Tab::new(None, String::new()))];

        Self {
            inner: LiteMap::from_iter(tabs.into_iter()),
            current,
        }
    }

    pub fn tab_list(&self) -> Vec<&str> {
        self.inner.values().map(Tab::name).collect()
    }

    pub fn current(&mut self) -> &mut Tab {
        &mut self.inner[&self.current]
    }

    pub fn open(&mut self, file_path: String) -> Result<(), io::Error> {
        let new_key = TabKey::new();
        let tmp_buf = fs::read_to_string(&file_path)?;
        let tab = Tab::new(Some(file_path), tmp_buf);
        self.inner.insert(new_key, tab);
        self.current = new_key;
        Ok(())
    }
}
