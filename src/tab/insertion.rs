use crate::config::trim_blank_lines;
use crate::config::autoclose_blocks;
use crate::interface::input::Jump;
use std::str::from_utf8;
use std::io::Write;
use super::*;

fn strip_cr<'a>(text: &'a str, eol_cr: &mut bool) -> &'a str {
    let (text, cr) = match text.strip_suffix('\r') {
        Some(text) => (text, true),
        None => (text, false),
    };

    *eol_cr = cr;
    text
}

fn indent_len(line: &str) -> usize {
    line.len() - line.trim_start().len()
}

impl Tab {
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
        let cursor = self.cursors[c];

        let line = &mut self.lines[cursor.y];
        let offset = line.len_until(cursor.x);
        line.buffer.insert_str(offset, text);
        line.set_dirty();

        let num_chars = text.chars().count();
        self.add_to_cursors(c, false, num_chars);
        self.check_line_highlighting(cursor.y);
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
            must_highlight: true,
            must_draw: true,
            eol_cr,
        };

        let old_y = cursor.y;
        self.lines.insert(old_y + 1, new_line);

        self.add_to_cursors(c, true, 0);
        self.set_lines_dirty(old_y);
    }

    pub(super) fn insert_text_cursor(&mut self, c: usize, text: &str) {
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

    pub fn insert_text(&mut self, text: &str) {
        self.prepare_insertion();
        self.erase_selection();

        for c in 0..self.cursors.len() {
            self.insert_text_cursor(c, text);
        }

        self.modified = true;
    }

    fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let text = c.encode_utf8(&mut buf);
        self.insert_text(text);
    }

    pub fn smart_insert_char(&mut self, c: char) {
        self.insert_char(c);

        if !autoclose_blocks() {
            return;
        }

        let special = [
            ('(', ')'),
            ('{', '}'),
            ('[', ']'),
        ];

        for (open, close) in special {
            if c != open { continue; }

            let one_back = Jump {
                select: false,
                forward: false,
                word_wise: false,
            };

            self.insert_char(close);
            self.horizontal_jump(one_back);
            break;
        }
    }

    pub fn insert_tab(&mut self) {
        let tab_string = take(&mut self.tab_string);
        self.insert_text(&tab_string);
        self.tab_string = tab_string;
    }

    pub fn smart_carriage_return(&mut self) {
        let blank = |s: &str| s.trim_start().is_empty();
        let block_syms = ["()", "{}", "[]"];
        const CAP: usize = 64;

        self.prepare_insertion();
        self.erase_selection();

        let num_cursors = self.cursors.len();
        let one_cursor = num_cursors == 1;

        for c in 0..num_cursors {
            let cursor = self.cursors[c];
            let line = &mut self.lines[cursor.y];
            let blank_line = blank(&line.buffer);

            let must_open = {
                let line_ends_with = |pair: &&str| line.buffer.ends_with(pair);
                let end_with_block = block_syms.iter().any(line_ends_with);
                let cursor_inbetween = line.len_chars() == (cursor.x + 1);
                end_with_block && cursor_inbetween
            };

            let indent = {
                let indent_len = indent_len(&line.buffer);
                let cursor_offset = line.len_until(cursor.x);
                let taken_len = usize::min(indent_len, cursor_offset);
                &line.buffer[..taken_len]
            };

            let crlf_i = line.eol_cr as usize;
            let crlf = ["\n", "\r\n"][crlf_i];

            let mut buf = [b'-'; CAP];
            let text = {
                let _ = write!(buf.as_mut_slice(), "{crlf}{indent}");
                let utf8 = from_utf8(&buf).unwrap();
                let len = utf8.find('-').unwrap_or(CAP);
                &utf8[..len]
            };

            if trim_blank_lines() && blank_line && one_cursor {
                self.backspace(c, cursor.x);
            }

            self.insert_text_cursor(c, text);

            if must_open {
                self.insert_text_cursor(c, text);

                // we just inserted two newlines, this must be ok?
                self.cursors[c].y -= 1;

                let tab_string = take(&mut self.tab_string);
                self.insert_text_cursor(c, &tab_string);
                self.tab_string = tab_string;
            }
        }

        self.modified = true;
    }
}
