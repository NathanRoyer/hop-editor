use std::{io, fs};

struct Entry {
    name_or_path: String,
    depth: usize,
}

pub struct FileTree {
    entries: Vec<Entry>,
    scroll: isize,
}

impl Entry {
    fn name(&self) -> &str {
        let mut pre_len = 0;

        if self.is_trunk() {
            let dec = self.is_dir() as usize;
            let base_len = self.name_or_path.len() - dec;
            let base = &self.name_or_path[..base_len];

            if let Some((pre, _)) = base.rsplit_once('/') {
                pre_len = pre.len() + 1;
            }
        }

        &self.name_or_path[pre_len..]
    }

    fn line(&self) -> (usize, &str) {
        (self.depth * 3, self.name())
    }

    fn is_trunk(&self) -> bool {
        self.depth == 0
    }

    fn is_dir(&self) -> bool {
        self.name_or_path.ends_with('/')
    }
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            scroll: 0,
        }
    }

    pub fn add_folder(&mut self, mut path: String) {
        while path.ends_with("//") {
            path.pop();
        }

        if !path.ends_with('/') {
            path.push('/');
        }

        let trunk = Entry {
            name_or_path: path,
            depth: 0,
        };

        self.entries.push(trunk);
    }

    fn get_path(&self, i: usize) -> Option<String> {
        let mut entry = self.entries.get(i)?;
        let mut depth = entry.depth;
        let mut parts = Vec::new();
        let mut j = i;

        while entry.depth > 0 {
            if entry.is_dir() {
                if entry.depth < depth {
                    parts.insert(0, entry.name_or_path.as_str());
                    depth = entry.depth;
                }
            }

            if parts.is_empty() {
                parts.push(entry.name_or_path.as_str());
            }

            j -= 1;
            entry = self.entries.get(j)?;
        };

        parts.insert(0, entry.name_or_path.as_str());
        Some(parts.join(""))
    }

    pub fn click(&mut self, index: u16) -> Option<String> {
        let i = self.scroll + (index as isize);

        let Ok(i) = usize::try_from(i) else {
            return None;
        };

        let Some(entry) = self.entries.get(i) else {
            return None;
        };

        if !entry.is_dir() {
            return self.get_path(i);
        }

        let depth = entry.depth + 1;
        let suffix = self.entries.split_off(i + 1);

        let is_unfolded = match suffix.first() {
            Some(e) => e.depth == depth,
            None => false,
        };

        if is_unfolded {
            let iter = suffix
                .into_iter()
                .skip_while(|e| e.depth >= depth);

            self.entries.extend(iter);
        } else {
            let dir_path = self.get_path(i)?;

            if read_dir(dir_path, &mut self.entries, depth).is_err() {
                // todo: handle error
                return None;
            };

            self.entries.extend(suffix);
        }

        None
    }

    pub fn line(&self, index: u16) -> Option<(usize, &str)> {
        let i = self.scroll + (index as isize);
        let i = usize::try_from(i).ok()?;
        self.entries.get(i).map(Entry::line)
    }

    pub fn check_overscroll(&mut self, tree_height: u16) {
        let max = self.entries.len().saturating_sub(1) as isize;

        if self.scroll > max {
            self.scroll = max;
        }

        let max = tree_height.saturating_sub(1) as isize;

        if self.scroll < -max {
            self.scroll = -max;
        }
    }

    pub fn scroll(&mut self, delta: isize) {
        self.scroll += delta;
    }
}

fn read_dir(dir_path: String, entries: &mut Vec<Entry>, depth: usize) -> io::Result<()> {
    let mut empty = true;

    for item in fs::read_dir(dir_path)? {
        let Ok(item) = item else {
            continue;
        };

        let Ok(mut name_or_path) = item.file_name().into_string() else {
            continue;
        };

        let Ok(ft) = item.file_type() else {
            continue;
        };

        if ft.is_dir() {
            name_or_path.push('/');
        }

        let entry = Entry {
            name_or_path,
            depth,
        };

        entries.push(entry);
        empty = false;
    }

    if empty {
        let entry = Entry {
            name_or_path: String::from("<empty>"),
            depth,
        };

        entries.push(entry);
    }

    Ok(())
}
