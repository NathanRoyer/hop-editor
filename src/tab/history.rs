use super::*;

impl Tab {
    fn log(&mut self, before: Edition) {
        if self.disable_history {
            return;
        }

        if self.history.last().map(|s| s.before) == Some(before) {
            return;
        }

        self.rebuild();

        let snapshot = Snapshot {
            cursors: self.cursors.clone(),
            buffer: self.tmp_buf.clone(),
            before,
        };

        self.history.push(snapshot);
    }

    pub fn prepare_insertion(&mut self) {
        self.log(Edition::Insertion)
    }

    pub fn prepare_deletion(&mut self) {
        self.log(Edition::Deletion)
    }

    pub fn undo(&mut self) {
        if let Some(snapshot) = self.history.pop() {
            let mut line = Line::default();
            let cursor = Cursor::new(0);
            line.dirty = true;

            self.lines.clear();
            self.lines.push(line);

            self.cursors.clear();
            self.cursors.push(cursor);

            self.disable_history = true;
            self.insert_text(&snapshot.buffer);
            self.disable_history = false;

            self.cursors.clear();
            self.cursors.extend(snapshot.cursors);
            self.highlight();
        }
    }
}
