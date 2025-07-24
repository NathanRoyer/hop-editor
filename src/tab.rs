use std::sync::atomic::{AtomicU64, Ordering};
use std::mem::{swap, take};

use crate::syntax::Range;

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
    fn dummy() -> Self {
        Self {
            file_path: None,
            tmp_buf: String::new(),

            // state
            ranges: vec![Range::default()],
            lines: vec![Line::default()],
            scroll: 0,
            target_x: None,
            cursor_x: 0,
            cursor_y: 0,
            cursor_r: 0,

            // settings
            tab_lang: String::from("default"),
            tab_width: 4,
        }
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

        self.cursor_x += text.len();
        self.target_x.take();
    }

    fn line_feed(&mut self, mut eol_cr: bool) {
        let range = &mut self.ranges[self.cursor_r];
        range.len += 1 + (eol_cr as usize);

        let line = &mut self.lines[self.cursor_y];
        swap(&mut line.eol_cr, &mut eol_cr);

        let new_line = Line {
            buffer: String::new(),
            eol_cr,
        };

        self.cursor_y += 1;
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
}

impl TabMap {
    pub fn new() -> Self {
        let current = TabKey::new();
        let tabs = [(current, Tab::dummy())];

        Self {
            inner: LiteMap::from_iter(tabs.into_iter()),
            current,
        }
    }

    pub fn tab_list(&self) -> Vec<&str> {
        self.inner.values().map(Tab::name).collect()
    }
}
