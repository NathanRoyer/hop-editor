use super::*;

impl Tab {
    pub fn highlight(&mut self) {
        let Some(syntax) = self.syntax.as_ref() else {
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

    pub fn prepare_draw(&mut self, y: u16) -> Option<(usize, bool)> {
        let i = self.line_index(y)?;
        let line = self.lines.get_mut(i)?;
        Some((i, take(&mut line.dirty)))
    }

    pub fn line_data<'a>(
        &'a mut self,
        index: usize,
        part_buf: &mut Vec<TextPart>,
        sel_buf: &mut Vec<Selection>,
        cursors: &mut Vec<usize>,
    ) -> DirtyLine<'a> {
        let latest = self.latest_cursor();
        let mut horizontal_scroll = 0;
        let tab_width_m1 = self.tab_width_m1;
        let line = &mut self.lines[index];
        let text = &line.buffer;
        let len = text.len();
        let mut covered = 0;

        // todo: maybe remove this useless copying
        for range in line.ranges.iter() {
            let mode_str = range.mode.as_str();
            part_buf.push((mode_str, range.len));
            covered += range.len;
        }

        // abnormal
        if covered < len {
            let missing = len - covered;
            part_buf.push(("wspace", missing));
        }

        for (c, cursor) in self.cursors.iter().enumerate() {
            let forward_sel = cursor.sel_y < 0;

            if cursor.covers(index) {
                sel_buf.clear();
                let len = line.len_chars();
                sel_buf.push(Selection::new(0, len));
                return DirtyLine { horizontal_scroll, tab_width_m1, text };
            }

            if cursor.y == index {
                cursors.push(cursor.x);

                if c == latest {
                    horizontal_scroll = self.h_scroll;
                }

                if cursor.sel_y != 0 {
                    sel_buf.push(line.half_select(forward_sel, cursor.x));
                } else if cursor.sel_x < 0 {
                    let orig_x = cursor.x - ((-cursor.sel_x) as usize);
                    let len = cursor.x - orig_x;
                    sel_buf.push(Selection::new(orig_x, len));
                } else if cursor.sel_x > 0 {
                    let end_x = cursor.x + (cursor.sel_x as usize);
                    let len = end_x - cursor.x;
                    sel_buf.push(Selection::new(cursor.x, len));
                }
            }

            if cursor.touches(index) {
                if let Some(orig_x) = cursor.x.checked_add_signed(cursor.sel_x) {
                    // confirm!("orig_x: {orig_x}");
                    sel_buf.push(line.half_select(!forward_sel, orig_x));
                }
            }
        }

        DirtyLine { horizontal_scroll, tab_width_m1, text }
    }

    pub fn check_overscroll(&mut self) {
        let max = self.lines.len().saturating_sub(1);

        if self.v_scroll > max {
            self.v_scroll = max;
        }
    }

    pub fn scroll(&mut self, delta: isize) {
        self.v_scroll = self.v_scroll.checked_add_signed(delta).unwrap_or(0);
        self.set_fully_dirty();
    }

    pub fn ensure_cursor_visible(&mut self, width: usize, height: usize) {
        let c = self.latest_cursor();
        let cursor = &self.cursors[c];

        let max_x = match self.h_scroll {
            0 => width.saturating_sub(1),
            n => width.saturating_sub(2) + n,
        };

        let max_y = self.v_scroll + height;

        let invisible_x = cursor.x < self.h_scroll || max_x <= cursor.x;
        let invisible_y = cursor.y < self.v_scroll || max_y <= cursor.y;

        if invisible_x {
            // only applies to latest cursor
            let line = &self.lines[cursor.y];
            let x = line.cells_until(cursor.x, self.tab_width_m1);
            self.h_scroll = x.saturating_sub(width / 2);
        }

        if invisible_y {
            self.v_scroll = cursor.y.saturating_sub(height / 2);
        }

        if invisible_x | invisible_y {
            // maybe todo do better for _x
            self.set_fully_dirty();
        }
    }
}
