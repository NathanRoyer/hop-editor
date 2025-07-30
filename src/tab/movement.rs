use super::*;

impl Tab {
    pub fn vertical_jump(&mut self, delta: isize, select: bool) {
        self.unselect_if_not(select, None);

        for c in 0..self.cursors.len() {
            let char_w = |c| match c {
                '\t' => self.tab_width_m1 + 1,
                _ => 1,
            };

            // to-do: check if we're not going back and
            // forth for nothing with cursor indices
            let cursor = &mut self.cursors[c];
            let line = &self.lines[cursor.y];
            let i = line.len_until(cursor.x);
            let x = line.buffer[..i].chars().map(char_w).sum();
            let cx_backup = cursor.x as isize;

            let y = cursor.y as isize + delta;
            if let Ok(y) = usize::try_from(y) {
                if y < self.lines.len() {
                    self.seek_in_line(c, y, x);
                }
            }

            if select {
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

    pub fn horizontal_jump(&mut self, delta: isize, select: bool) {
        type Sig = (usize, fn(&mut Tab, usize, bool));
        self.unselect_if_not(select, Some(delta < 0));

        let (num_iter, callback): Sig = match delta < 0 {
            true => ((-delta) as usize, Self::backoff_cursor_once),
            false => (delta as usize, Self::advance_cursor_once),
        };

        for c in 0..self.cursors.len() {
            for _ in 0..num_iter {
                let y = self.cursors[c].y;
                self.lines[y].dirty = true;
                callback(self, c, select);
            }

            let y = self.cursors[c].y;
            self.lines[y].dirty = true;
        }

        self.check_cursors();
    }

    fn seek_in_line(&mut self, c: usize, y: usize, mut x: usize) {
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
        self.unselect_if_not(append, None);
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

    pub fn unselect_if_not(&mut self, select: bool, jump_dir: Option<bool>) {
        if select {
            return;
        }

        for cursor in self.cursors.iter_mut() {
            if cursor.sel_x == 0 && cursor.sel_y == 0 {
                continue;
            }

            for (i, line) in self.lines.iter_mut().enumerate() {
                if cursor.y == i || cursor.covers(i) || cursor.touches(i) {
                    line.dirty = true;
                }
            }

            if let Some(dir) = jump_dir {
                cursor.sel_jump(dir);
            }

            cursor.sel_x = 0;
            cursor.sel_y = 0;
        }
    }
}