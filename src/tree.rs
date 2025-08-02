use crate::config::hide_folder;
use std::{io, fs, cmp};
use std::fmt::Write;
use crate::confirm;

// syms: ▷▽▶▼;

#[derive(Clone, Debug, PartialEq, Eq)]
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

    fn get_path(&self, i: usize) -> String {
        let mut entry = &self.entries[i];
        let mut depth = entry.depth;
        let mut parts = Vec::new();
        let mut j = i;

        while entry.depth > 0 {
            if entry.is_dir() && entry.depth < depth {
                parts.insert(0, entry.name_or_path.as_str());
                depth = entry.depth;
            }

            if parts.is_empty() {
                parts.push(entry.name_or_path.as_str());
            }

            j -= 1;
            entry = self.entries.get(j).expect("invalid structure");
        };

        parts.insert(0, entry.name_or_path.as_str());
        parts.join("")
    }

    pub fn toggle_dir(&mut self, i: usize, unfold_only: bool) {
        let entry = &mut self.entries[i];
        let inc_depth = entry.depth + 1;
        let suffix = self.entries.split_off(i + 1);

        let is_unfolded = match suffix.first() {
            Some(next) => next.depth == inc_depth,
            None => false,
        };

        if is_unfolded & unfold_only {
            self.entries.extend(suffix);
        } else if is_unfolded {
            let iter = suffix.into_iter();
            let crit = |e: &Entry| e.depth >= inc_depth;
            self.entries.extend(iter.skip_while(crit));
        } else {
            let dir_path = self.get_path(i);
            let i = self.entries.len();

            if let Err(error) = read_dir(dir_path, &mut self.entries, inc_depth) {
                confirm!("failed to read directory: {error:?}");
                self.entries.truncate(i);
            };

            self.entries[i..].sort();
            self.entries.extend(suffix);
        }
    }

    pub fn click(&mut self, index: u16) -> Option<String> {
        let i = self.scroll + (index as isize);
        let i = usize::try_from(i).ok()?;
        self.toggle_or_open(i)
    }

    pub fn line(&self, buf: &mut String, index: u16) -> Option<usize> {
        buf.clear();
        let y = self.scroll + (index as isize);
        let i = usize::try_from(y).ok()?;
        let entry = self.entries.get(i)?;
        let is_dir = entry.is_dir();
        let name = entry.name();

        let is_unfolded = self
            .entries
            .get(i + 1)
            .is_some_and(|next| next.depth > entry.depth);

        let sym = match (is_dir, is_unfolded) {
            (true, false) => '▷',
            (true, true) => '▼',
            (false, _) => ' ',
        };

        let indent = entry.depth * 3;
        let _ = write!(buf, "{:1$}{sym} {name}", "", indent);

        Some(i)
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

    pub fn reveal_path(&mut self, mut path: &str) -> Option<usize> {
        let mut found_trunk = false;
        let mut i = 0;

        while i < self.entries.len() {
            let entry = &self.entries[i];
            let mut open_dir = false;

            if let Some(next) = path.strip_prefix(&entry.name_or_path) {
                found_trunk |= entry.is_trunk();

                if found_trunk {
                    path = next;
                    open_dir = entry.is_dir();
                }

                if path.is_empty() {
                    return Some(i);
                }

                if open_dir {
                    self.toggle_dir(i, true);
                }
            }

            i += 1;
        }

        None
    }

    pub fn toggle_or_open(&mut self, i: usize) -> Option<String> {
        if self.entries.get(i)?.is_dir() {
            self.toggle_dir(i, false);
            None
        } else {
            Some(self.get_path(i))
        }
    }

    pub fn enter_dir(&mut self, i: &mut usize) {
        if !self.entries[*i].is_dir() {
            return;
        }

        self.toggle_dir(*i, true);
        *i += 1;
    }

    pub fn leave_dir(&mut self, i: &mut usize) {
        let depth = self.entries[*i].depth;
        let Some(target) = depth.checked_sub(1) else {
            return;
        };

        while self.entries[*i].depth != target {
            *i -= 1;
        }
    }

    pub fn up_down(&mut self, i: &mut usize, delta: isize) {
        if let Some(next) = i.checked_add_signed(delta) {
            if next < self.entries.len() {
                *i = next;
            }
        }
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

        let hidden = hide_folder(&name_or_path);

        let (Ok(ft), false) = (item.file_type(), hidden) else {
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

impl PartialOrd for Entry {
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// dirs < files
impl Ord for Entry {
    fn cmp(&self, other: &Self) -> cmp::Ordering {
        match (self.is_dir(), other.is_dir()) {
            (true, false) => cmp::Ordering::Less,
            (false, true) => cmp::Ordering::Greater,
            _ => self.name().cmp(other.name()),
        }
    }
}
