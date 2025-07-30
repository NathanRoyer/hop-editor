use super::*;

impl Tab {
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

        for cursor in &self.cursors {
            let forward_sel = cursor.sel_y < 0;

            if cursor.covers(index) {
                sel_buf.clear();
                let len = line.len_chars();
                sel_buf.push(Selection::new(0, len));
                return DirtyLine { tab_width_m1, text };
            }

            if cursor.y == index {
                cursors.push(cursor.x);

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
                    sel_buf.push(line.half_select(!forward_sel, orig_x));
                }
            }
        }

        DirtyLine { tab_width_m1, text }
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
}
