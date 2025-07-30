use super::*;

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
        for c in 0..self.cursors.len() {
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
}
