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
        line.dirty = true;

        self.check_line_highlighting(cursor.y);
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

    pub fn insert_char(&mut self, c: char) {
        let mut buf = [0u8; 4];
        let text = c.encode_utf8(&mut buf);
        self.insert_text(text);
    }

    pub fn smart_carriage_return(&mut self) {
        const CAP: usize = 64;

        let c = self.latest_cursor();
        let index = self.cursors[c].y;
        let line = &self.lines[index];

        let crlf_i = line.eol_cr as usize;
        let crlf = ["\n", "\r\n"][crlf_i];

        let indent_len = indent_len(&line.buffer);
        let indent = &line.buffer[..indent_len];

        let mut buf = [b'-'; CAP];
        let _ = write!(buf.as_mut_slice(), "{crlf}{indent}");
        let text = from_utf8(&buf).unwrap();
        let len = text.find('-').unwrap_or(CAP);

        self.insert_text(&text[..len]);
    }
}
