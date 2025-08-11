use std::{io, fs, cmp};
use std::fmt::Write;

use crate::interface::menu::{MenuItem, context_menu};
use crate::{alert, confirm, prompt};
use crate::config::hide_folder;

// syms: ▷▽▶▼;

#[derive(Clone, Debug, PartialEq, Eq)]
struct Entry {
    name_or_path: String,
    depth: usize,
}

pub struct FileTree {
    entries: Vec<Entry>,
    scroll: usize,
}

fn pop_dir_slash(text: &str) -> &str {
    let dec = text.ends_with('/') as usize;
    let base_len = text.len() - dec;
    &text[..base_len]
}

impl Entry {
    fn name(&self) -> &str {
        let mut pre_len = 0;

        if self.is_trunk() {
            let base = pop_dir_slash(&self.name_or_path);

            if let Some(index) = base.rfind('/') {
                pre_len = index + 1;
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
                alert!("failed to read directory: {error:?}");
                self.entries.truncate(i);
            };

            self.entries[i..].sort();
            self.entries.extend(suffix);
        }
    }

    pub fn click(&mut self, index: u16) -> Option<String> {
        self.toggle_or_open(index as usize + self.scroll)
    }

    pub fn line(&self, buf: &mut String, index: u16) -> Option<usize> {
        let i = index as usize + self.scroll;
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
        let _ = write!(buf, " {:1$}{sym} {name}", "", indent);

        Some(i)
    }

    pub fn check_overscroll(&mut self) {
        let max = self.entries.len().saturating_sub(1);

        if self.scroll > max {
            self.scroll = max;
        }
    }

    pub fn scroll(&mut self, delta: isize) {
        self.scroll = self.scroll.checked_add_signed(delta).unwrap_or(0);
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

    pub fn right_click<F: Fn(&str) -> bool>(
        &mut self,
        x: u16,
        y: u16,
        entry: u16,
        is_in_use: F,
    ) {
        use MenuItem::*;

        let i = entry as usize + self.scroll;
        let Some(entry) = self.entries.get(i) else {
            return;
        };

        let options = match entry.is_dir() {
            true => [NewFile, NewDir, Rename, Delete].as_slice(),
            false => [Rename, Delete].as_slice(),
        };

        if let Some(action) = context_menu(x, y, options) {
            if let Err(error) = self.affect(i, action, is_in_use) {
                alert!("Failed: {error:?}");
            }
        }
    }

    fn affect<F: Fn(&str) -> bool>(
        &mut self,
        i: usize,
        action: MenuItem,
        is_in_use: F,
    ) -> io::Result<()> {
        use MenuItem::*;

        let mut old_path = self.get_path(i);
        let in_use = is_in_use(&old_path);
        let entry = &mut self.entries[i];

        let old_name = pop_dir_slash(entry.name()).to_string();

        if [Rename, Delete].contains(&action) && in_use {
            alert!("Not possible!\nAt least one of your tabs relies on this path.");
            return Ok(());
        }

        match action {
            Rename => {
                let Some(new_name) = prompt!("New name for {old_name:?}:") else {
                    return Ok(());
                };

                let mut new_path = old_path.clone();
                replace_last(&mut new_path, &old_name, &new_name);
                fs::rename(old_path, new_path)?;
                replace_last(&mut entry.name_or_path, &old_name, &new_name);
            },
            Delete => self.delete(i)?,
            NewDir => {
                let Some(mut dir_name) = prompt!("Name of new directory in {old_name:?}:") else {
                    return Ok(());
                };

                dir_name.push('/');
                old_path += &dir_name;
                fs::create_dir(old_path)?;
                self.insert_entry(i, dir_name);
            },
            NewFile => {
                let Some(file_name) = prompt!("Name of new file in {old_name:?}:") else {
                    return Ok(());
                };
 
                old_path += &file_name;
                fs::write(old_path, "")?;
                self.insert_entry(i, file_name);
            },
            other => _ = alert!("Bad Code Path ({other:?})"),
        }

        Ok(())
    }

    fn insert_entry(&mut self, mut i: usize, name: String) {
        use cmp::Ordering::{Less, Equal};
        let parent = &self.entries[i];
        let inc_depth = parent.depth + 1;
        let entry = Entry {
            name_or_path: name,
            depth: inc_depth,
        };

        loop {
            let next = i + 1;
            let Some(neighbor) = self.entries.get(next) else {
                break;
            };

            match neighbor.depth.cmp(&inc_depth) {
                Less => break,
                Equal if neighbor >= &entry => break,
                _other => i = next,
            }
        }

        self.entries.insert(i, entry);
    }

    fn delete(&mut self, i: usize) -> io::Result<()> {
        let old_path = self.get_path(i);
        let entry = &self.entries[i];
        let inc_depth = entry.depth + 1;
        let mut keep_going = true;

        if !confirm!("Really delete this?\n{old_path:?}") {
            return Ok(());
        }

        match entry.is_dir() {
            true => fs::remove_dir_all(old_path)?,
            false => fs::remove_file(old_path)?,
        }

        while keep_going {
            self.entries.remove(i);

            match self.entries.get(i) {
                None => keep_going = false,
                Some(entry) => keep_going = entry.depth >= inc_depth,
            }
        }

        Ok(())
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

fn replace_last(dst: &mut String, from: &str, to: &str) {
    let start = dst
        .rfind(from)
        .expect("replace_last: no occurence!");

    let stop = start + from.len();
    dst.replace_range(start..stop, to);
}
