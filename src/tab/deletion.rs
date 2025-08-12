use super::*;

impl Tab {
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

    fn merge_with_prev_line(&mut self, c: usize, this_y: usize, prev_y: usize) {
        let mut line = self.lines.remove(this_y);
        let buf = take(&mut line.buffer);

        let prev = &mut self.lines[prev_y];
        let x_add = prev.len_chars();
        prev.buffer += buf.as_str();

        self.set_lines_dirty(prev_y);
        self.backspace_cursor(c, true, x_add as isize);
    }

    fn backspace(&mut self, c: usize, mut num_chars: usize) {
        let simple_backspace = num_chars == 1;

        while self.cursors[c].x < num_chars {
            let this_y = self.cursors[c].y;

            if let Some(prev_y) = this_y.checked_sub(1) {
                self.merge_with_prev_line(c, this_y, prev_y);
            };

            num_chars -= 1;
        }

        if num_chars == 0 {
            return;
        }

        // mutable copy! not mut ref
        let cursor = self.cursors[c];
        let line = &mut self.lines[cursor.y];

        let old_i = line.len_until(cursor.x);

        // erase whole soft tab
        let slice = &line.buffer[..old_i];
        let is_tab = slice.ends_with(&self.tab_string);

        if simple_backspace && is_tab {
            num_chars = self.tab_string.chars().count();
        }

        let new_i = line.len_until(cursor.x - num_chars);

        line.buffer.replace_range(new_i..old_i, "");
        line.set_dirty();

        self.check_line_highlighting(cursor.y);
        self.backspace_cursor(c, false, -(num_chars as isize));
    }

    pub fn backspace_once(&mut self, forward: bool) {
        if !self.erase_selection() {
            self.prepare_deletion();

            if forward {
                self.horizontal_jump(1, false);
            }

            for c in 0..self.cursors.len() {
                self.backspace(c, 1);
            }
        }

        self.modified = true;
    }

    pub fn erase_selection(&mut self) -> bool {
        if !self.has_selections() {
            return false;
        }

        self.prepare_deletion();
        let range = 0..self.cursors.len();
        for c in range.rev() {
            let cursor = &mut self.cursors[c];

            // ensures sel is earlier than cursor
            cursor.sel_jump(false);

            loop {
                let cursor = &mut self.cursors[c];

                if cursor.sel_y == 0 {
                    break;
                }

                let x = cursor.x;
                cursor.sel_y += 1;
                cursor.sel_x += x as isize;
                self.backspace(c, x + 1);

                let cursor = &mut self.cursors[c];
                cursor.sel_x -= cursor.x as isize;
            }

            let cursor = &mut self.cursors[c];
            let sel_x = take(&mut cursor.sel_x);
            self.backspace(c, -sel_x as usize);
        }

        // todo: do better
        self.set_fully_dirty();

        true
    }
}
