use crate::interface::input::Jump;
use super::*;

#[derive(Debug, Default, Copy, Clone)]
struct TokBoundaries {
    word_char_offset: usize,
    word_chars: usize,
}

impl Tab {
    pub fn vertical_jump(&mut self, jump: Jump) {
        self.unselect_if_not(jump.select, None);

        let delta = match jump.forward {
            true => 1,
            false => -1,
        };

        for c in 0..self.cursors.len() {
            // to-do: check if we're not going back and
            // forth for nothing with cursor indices
            let cursor = &mut self.cursors[c];
            let line = &self.lines[cursor.y];
            let x = line.cells_until(cursor.x, self.tab_width_m1);
            let cx_backup = cursor.x as isize;

            let y = cursor.y as isize + delta;
            if let Ok(y) = usize::try_from(y) {
                if y < self.lines.len() {
                    self.seek_in_line(c, y, x);
                }
            }

            if jump.select {
                let cursor = &mut self.cursors[c];
                cursor.sel_x += cx_backup - cursor.x as isize;
                cursor.sel_y -= delta;
            }
        }

        self.check_cursors();
    }

    fn backoff_cursor_once(&mut self, c: usize, select: bool) {
        let cursor = &mut self.cursors[c];

        if cursor.x != 0 {
            cursor.x -= 1;
            cursor.sel_x += select as isize;
        } else if cursor.y != 0 {
            cursor.y -= 1;
            let line = &self.lines[cursor.y];
            cursor.x = line.len_chars();

            if select {
                cursor.sel_x -= cursor.x as isize;
                cursor.sel_y += 1;
            }
        }
    }

    fn advance_cursor_once(&mut self, c: usize, select: bool) {
        let cursor = &mut self.cursors[c];

        let line = &self.lines[cursor.y];
        let lines = self.lines.len();
        let next_x = cursor.x + 1;
        let next_y = cursor.y + 1;
        let chars = line.len_chars();

        let has_next_line = next_y < lines;
        let overflow = next_x > chars;

        if overflow & has_next_line {
            cursor.x = 0;
            cursor.y = next_y;

            if select {
                cursor.sel_y -= 1;
                cursor.sel_x += chars as isize;
            }
        } else if !overflow {
            cursor.x = next_x;
            cursor.sel_x -= select as isize;
        }
    }

    fn hor_jump_cursor(&mut self, c: usize, delta: isize, select: bool) {
        type Sig = (usize, fn(&mut Tab, usize, bool));

        let (num_iter, callback): Sig = match delta < 0 {
            true => ((-delta) as usize, Self::backoff_cursor_once),
            false => (delta as usize, Self::advance_cursor_once),
        };

        for _ in 0..num_iter {
            let y = self.cursors[c].y;
            self.lines[y].must_draw = true;
            callback(self, c, select);
        }

        let y = self.cursors[c].y;
        self.lines[y].must_draw = true;
    }

    pub fn horizontal_jump(&mut self, jump: Jump) {
        self.unselect_if_not(jump.select, Some(!jump.forward));

        for c in 0..self.cursors.len() {
            let delta;

            if jump.word_wise {
                let mut boundaries = self.find_word_boundaries(c);
                let mut cursor = &mut self.cursors[c];

                let mut chars_end = boundaries.word_char_offset + boundaries.word_chars;
                let mut cursor_offset = cursor.x - boundaries.word_char_offset;

                if !jump.forward && cursor_offset == 0 {
                    cursor.x = cursor.x.saturating_sub(1);

                    boundaries = self.find_word_boundaries(c);
                    cursor = &mut self.cursors[c];

                    chars_end = boundaries.word_char_offset + boundaries.word_chars;
                    cursor_offset = cursor.x - boundaries.word_char_offset;
                }

                delta = match jump.forward {
                    true => (chars_end - cursor.x) as isize,
                    false => -(cursor_offset as isize),
                };
            } else {
                delta = match jump.forward {
                    true => 1,
                    false => -1,
                };
            }

            self.hor_jump_cursor(c, delta, jump.select);
        }

        self.check_cursors();
    }

    fn seek_in_line(&mut self, c: usize, y: usize, mut x: usize) {
        let cursor = &mut self.cursors[c];
        self.lines[cursor.y].must_draw = true;
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
        self.lines[y].must_draw = true;
    }

    pub fn seek(&mut self, x: u16, y: u16, append: bool) {
        let Some(y) = self.line_index(y) else {
            return;
        };

        let mut x = x as usize;
        let c = self.latest_cursor();
        let cursor = &self.cursors[c];

        if cursor.y == y {
            x += self.h_scroll.saturating_sub(1);
        }

        let same_xy = (cursor.x == x) & (cursor.y == y);

        if same_xy && !cursor.selects() {
            self.auto_select();
            return;
        }

        self.unselect_if_not(append, None);

        if !append {
            for cursor in &self.cursors {
                self.lines[cursor.y].must_draw = true;
            }

            self.cursors.clear();
        }

        let id = self.cursors.len();
        self.cursors.push(Cursor::new(id));
        self.seek_in_line(id, y, x);
        self.check_cursors();
    }

    pub fn latest_cursor(&mut self) -> usize {
        let iter = self.cursors.iter().enumerate();
        iter.max_by_key(|(_, c)| c.id).unwrap().0
    }

    pub fn drag_to(&mut self, x: u16, y: u16) {
        let Some(y) = self.line_index(y) else {
            return;
        };

        let mut x = x as usize;
        let c = self.latest_cursor();
        let cursor = &mut self.cursors[c];

        if cursor.y == y {
            x += self.h_scroll.saturating_sub(1);
        }

        cursor.swap_sel_direction();
        let backup = *cursor;

        self.seek_in_line(c, y as usize, x as usize);

        let cursor = &mut self.cursors[c];
        cursor.sel_x = (backup.x as isize) - (cursor.x as isize);
        cursor.sel_y = (backup.y as isize) - (cursor.y as isize);

        self.check_cursors();
        self.set_lines_redraw();
    }

    fn unselect_if_not(&mut self, select: bool, jump_dir: Option<bool>) {
        if !select {
            self.unselect(jump_dir);
        }
    }

    pub fn unselect(&mut self, jump_dir: Option<bool>) {
        for cursor in self.cursors.iter_mut() {
            if !cursor.selects() {
                continue;
            }

            for (i, line) in self.lines.iter_mut().enumerate() {
                if cursor.y == i || cursor.covers(i) || cursor.touches(i) {
                    line.must_draw = true;
                }
            }

            if let Some(backwards) = jump_dir {
                cursor.sel_jump(backwards);
            }

            cursor.sel_x = 0;
            cursor.sel_y = 0;
        }
    }

    pub fn auto_select(&mut self) {
        let c = self.latest_cursor();

        if self.cursors[c].selects() {
            return self.find_next_occurence(c);
        }

        let boundaries = self.find_word_boundaries(c);
        let cursor = &mut self.cursors[c];

        let chars_end = boundaries.word_chars + boundaries.word_char_offset;

        cursor.sel_y = 0;
        cursor.sel_x = -(boundaries.word_chars as isize);
        cursor.x = chars_end;

        self.lines[cursor.y].must_draw = true;
    }

    fn find_word_boundaries(&mut self, c: usize) -> TokBoundaries {
        // is this necessary?
        self.highlight();

        let cursor = &self.cursors[c];
        let line = &self.lines[cursor.y];

        let mut boundaries = TokBoundaries::default();

        if self.syntax.is_some() {
            let mut byte_offset = 0;

            for range in &line.ranges {
                let end = byte_offset + range.len;
                let slice = &line.buffer[byte_offset..end];

                boundaries.word_chars = slice.chars().count();
                let next = boundaries.word_char_offset + boundaries.word_chars;

                if cursor.x < next {
                    break;
                }

                boundaries.word_char_offset = next;
                byte_offset = end;

                // in case the cursor is at EOL,
                // set the token len to zero
                boundaries.word_chars = 0;
            }
        } else {
            let wspace = &[' ', '\t'];
            let cursor_byte_offset = line.len_until(cursor.x);

            let before = &line.buffer[..cursor_byte_offset];
            let after = &line.buffer[cursor_byte_offset..];

            let pre = match before.rsplit_once(wspace) {
                Some((_, l_side)) => l_side.len(),
                None => before.len(),
            };

            let post = match after.split_once(wspace) {
                Some((r_side, _)) => r_side.len(),
                None => after.len(),
            };

            let start = cursor_byte_offset - pre;
            let end = cursor_byte_offset + post;

            let char_count = |s: &str| s.chars().count();

            boundaries.word_char_offset = char_count(&line.buffer[..start]);
            boundaries.word_chars = char_count(&line.buffer[start..end]);
        }

        boundaries
    }

    fn matches(&self, text: &str, x: usize, mut y: usize) -> bool {
        let line = &self.lines[y].buffer;
        let mut line_iter = line.chars().skip(x);
        let mut text_iter = text.chars();

        loop {
            let Some(text_char) = text_iter.next() else {
                break true;
            };

            let line_char = line_iter.next();
            let has_lf = line_char.is_none();
            let expect_lf = text_char == '\n';

            if expect_lf && has_lf {
                y += 1;

                line_iter = match self.lines.get(y) {
                    Some(line) => line.buffer.chars().skip(0),
                    None => break false,
                };

                continue;
            }

            if line_char != Some(text_char) {
                break false;
            }
        }
    }

    fn find(&self, text: &str, mut start_x: usize, start_y: usize) -> Option<(usize, usize)> {
        let lines = self.lines.len();

        for y in start_y..lines {
            let len = self.lines[y].len_chars();

            for x in start_x..=len {
                if self.matches(text, x, y) {
                    return Some((x, y));
                }
            }

            start_x = 0;
        }

        None
    }

    pub fn find_all(&mut self) {
        let c = self.latest_cursor();
        let cursor = &self.cursors[c];

        let maybe_text = if cursor.selects() {
            let mut dst = String::new();
            self.extract_selection(c, &mut dst);
            Some(dst)
        } else {
            prompt!("{}", crate::SEARCH_PROMPT)
        };

        if let Some(text) = maybe_text {
            self.locate(&text);
        }
    }

    pub fn locate(&mut self, text: &str) {
        let num_chars = text.chars().count() as isize;
        let mut cursor = Cursor::new(0);
        let mut c = 0;

        while let Some((x, y)) = self.find(&text, cursor.x, cursor.y) {
            if c == 0 {
                self.cursors.clear();
            }

            cursor = Cursor::new(c);
            cursor.x = x;
            cursor.y = y;
            self.cursors.push(cursor);
            self.hor_jump_cursor(c, num_chars, true);
            cursor = *self.cursors.last().unwrap();
            c += 1;
        }

        if c == 0 {
            alert!("No match in current tab.");
        }

        self.check_cursors();
        self.h_scroll = 0;
    }

    pub(super) fn extract_selection<T: AppendStr>(&self, c: usize, dst: &mut T) {
        let mut a = self.cursors[c];
        let mut b = a;

        a.sel_jump(true);
        b.sel_jump(false);

        for y in a.y..=b.y {
            let line = &self.lines[y];

            let start_x = match y == a.y {
                true => a.x,
                false => 0,
            };

            let (limit_x, addition) = match (y == b.y, line.eol_cr) {
                (true, _) => (b.x, ""),
                (false, true) => (line.len_chars(), "\r\n"),
                (false, false) => (line.len_chars(), "\n"),
            };

            let limit_len = line.len_until(limit_x);
            let start_len = line.len_until(start_x);

            let line = &line.buffer[start_len..limit_len];
            dst.append(line);
            dst.append(addition);
        }
    }

    fn find_next_occurence(&mut self, c: usize) {
        let mut text = String::new();
        self.extract_selection(c, &mut text);
        let mut cursor = self.cursors[c];
        let chars = text.chars().count();
        cursor.sel_jump(false);

        if let Some((x, y)) = self.find(&text, cursor.x, cursor.y) {
            let id = self.cursors.len();
            cursor.sel_jump(true);

            let new_cursor = Cursor {
                x,
                y,
                sel_x: 0,
                sel_y: 0,
                id,
            };

            self.cursors.push(new_cursor);
            self.hor_jump_cursor(id, chars as isize, true);
            self.check_cursors();
        }
    }

    pub fn line_seek(&mut self, to_start: bool, select: bool) {
        self.unselect_if_not(select, None);

        for c in 0..self.cursors.len() {
            let cursor = &self.cursors[c];
            let line = &self.lines[cursor.y];

            let target = match to_start {
                true => 0,
                false => line.len_chars() as isize,
            };

            let delta = target - (cursor.x as isize);
            self.hor_jump_cursor(c, delta, select);
        }

        self.check_cursors();
    }

    pub fn select_all(&mut self) {
        let Some(last) = self.lines.last() else {
            return;
        };

        let mut cursor = Cursor::new(0);

        cursor.x = last.len_chars();
        cursor.y = self.lines.len().saturating_sub(1);

        cursor.sel_x = -(cursor.x as isize);
        cursor.sel_y = -(cursor.y as isize);

        self.cursors.clear();
        self.cursors.push(cursor);
        self.set_lines_redraw();
    }
}

pub(super) trait AppendStr: Default {
    fn append(&mut self, text: &str);
}

impl AppendStr for String {
    fn append(&mut self, text: &str) {
        *self += text;
    }
}

impl AppendStr for usize {
    fn append(&mut self, text: &str) {
        *self += text.chars().count();
    }
}
