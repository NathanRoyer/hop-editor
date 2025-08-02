use super::*;

#[derive(Copy, Clone, PartialEq, Eq)]
enum Edition {
    Insertion,
    Deletion,
}

#[derive(Default)]
struct RawSnapshot {
    cursors: Vec<Cursor>,
    buffer: String,
}

struct Snapshot {
    raw: RawSnapshot,
    before: Edition,
}

pub struct History {
    pre_undo: Option<RawSnapshot>,
    inner: Vec<Snapshot>,
    len: Option<usize>,
}

impl History {
    pub fn new() -> Self {
        Self {
            pre_undo: None,
            inner: Vec::new(),
            len: None,
        }
    }

    pub fn activate(&mut self) {
        assert!(self.len.is_none());
        self.len = Some(0);
    }
}

impl Tab {
    fn raw_snapshot(&mut self) -> RawSnapshot {
        self.rebuild();

        RawSnapshot {
            cursors: self.cursors.clone(),
            buffer: self.tmp_buf.clone(),
        }
    }

    fn log(&mut self, before: Edition) {
        let Some(len) = self.history.len.as_mut() else {
            return;
        };

        if let Some(i) = len.checked_sub(1) {
            if self.history.inner[i].before == before {
                return;
            }
        }

        self.history.inner.truncate(*len);
        *len += 1;

        let snapshot = Snapshot {
            raw: self.raw_snapshot(),
            before,
        };

        self.history.inner.push(snapshot);
        self.history.pre_undo.take();
    }

    pub fn prepare_insertion(&mut self) {
        self.log(Edition::Insertion)
    }

    pub fn prepare_deletion(&mut self) {
        self.log(Edition::Deletion)
    }

    fn restore_snapshot(&mut self, snapshot: &RawSnapshot) {
        self.history.len.take();

        let mut line = Line::default();
        line.dirty = true;

        self.lines.clear();
        self.lines.push(line);

        self.cursors.clear();
        let insert_cursor = Cursor::new(0);
        self.cursors.push(insert_cursor);

        self.insert_text(&snapshot.buffer);

        self.cursors.clear();
        self.cursors.extend_from_slice(&snapshot.cursors);

        self.highlight();
    }

    pub fn undo(&mut self) {
        let Some(len) = self.history.len else {
            return;
        };

        let Some(last) = len.checked_sub(1) else {
            return;
        };

        if self.history.pre_undo.is_none() {
            self.history.pre_undo = Some(self.raw_snapshot());
        }

        let snapshot = take(&mut self.history.inner[last].raw);
        self.restore_snapshot(&snapshot);

        self.history.inner[last].raw = snapshot;
        self.history.len = Some(last);
    }

    pub fn redo(&mut self) {
        let Some(snapshot) = self.history.pre_undo.take() else {
            return;
        };

        self.restore_snapshot(&snapshot);
        let actual_len = self.history.inner.len();
        self.history.len = Some(actual_len);
    }
}
