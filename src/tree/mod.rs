#![allow(unused_variables)]

use std::{io, fs, cmp};
use std::fmt::Write;
use std::mem::take;

use crate::interface::menu::{MenuItem, context_menu};
use crate::{alert, confirm, prompt};
use crate::config::hide_folder;

use entry::{Options, EntryApi, TrunkApi};
use walker::PathWalker;
use local_fs::FsTrunk;

mod entry;
mod walker;
mod local_fs;

// syms: ▷▽▶▼;

pub struct FileTree {
    trunks: Vec<FsTrunk>,
    scroll: usize,
}

impl FileTree {
    pub fn new() -> Self {
        Self {
            trunks: Vec::new(),
            scroll: 0,
        }
    }

    pub fn add_local_folder(&mut self, path: String) {
        let trunk = FsTrunk::new(path);
        self.trunks.push(trunk);
    }

    pub fn toggle_dir(&mut self, i: usize, unfold_only: bool) {
    }

    pub fn click(&mut self, index: u16) -> Option<String> {
        self.toggle_or_open(index as usize + self.scroll)
    }

    fn len(&self) -> usize {
        self.trunks.iter().map(|t| t.len()).sum()
    }

    fn trunk(&self, offset: &mut usize) -> Option<&FsTrunk> {
        for trunk in self.trunks.iter() {
            if let Some(next) = offset.checked_sub(trunk.len()) {
                *offset = next;
            } else {
                return Some(trunk);
            }
        }

        None
    }

    fn trunk_mut(&mut self, offset: &mut usize) -> Option<&mut FsTrunk> {
        for trunk in self.trunks.iter_mut() {
            if let Some(next) = offset.checked_sub(trunk.len()) {
                *offset = next;
            } else {
                return Some(trunk);
            }
        }

        None
    }

    pub fn line(&self, buf: &mut String, index: u16) -> Option<usize> {
        let mut i = index as usize + self.scroll;
        let trunk = self.trunk(&mut i)?;
        let entry = trunk.get(i);
        let is_dir = entry.is_dir();
        let name = entry.name();
        let indent = entry.depth() * 3;

        let sym = match (is_dir, trunk.is_dir_open(i)) {
            (true, false) => '▷',
            (true, true) => '▼',
            (false, _) => ' ',
        };

        let _ = write!(buf, " {:1$}{sym} {name}", "", indent);

        Some(i)
    }

    pub fn check_overscroll(&mut self) {
        let max = self.len().saturating_sub(1);

        if self.scroll > max {
            self.scroll = max;
        }
    }

    pub fn scroll(&mut self, delta: isize) {
        self.scroll = self.scroll.checked_add_signed(delta).unwrap_or(0);
    }

    pub fn reveal_path(&mut self, mut path: &str) -> Option<usize> {
        None
    }

    pub fn toggle_or_open(&mut self, mut i: usize) -> Option<String> {
        let trunk = self.trunk_mut(&mut i)?;

        if trunk.get(i).is_dir() {
            match trunk.is_dir_open(i) {
                true => trunk.close_dir(i),
                false => trunk.open_dir(i),
            }

            None
        } else {
            Some(trunk.open_file(i))
        }
    }

    pub fn right_click<F: Fn(&str) -> bool>(
        &mut self,
        x: u16,
        y: u16,
        entry: u16,
        is_in_use: F,
    ) {
        /*use MenuItem::*;

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
        }*/
    }

    fn affect<F: Fn(&str) -> bool>(
        &mut self,
        i: usize,
        action: MenuItem,
        is_in_use: F,
    ) -> io::Result<()> {
        /*use MenuItem::*;

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
*/
        Ok(())
    }
/*
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
*/
    pub fn enter_dir(&mut self, i: &mut usize) {
        let mut j = *i;
        let trunk = self.trunk_mut(&mut j).unwrap();

        if !trunk.get(j).is_dir() {
            return;
        }

        trunk.open_dir(*i);
        *i += 1;
    }

    pub fn leave_dir(&mut self, i: &mut usize) {
        let mut j = *i;
        let trunk = self.trunk_mut(&mut j).unwrap();

        let depth = trunk.get(j).depth();
        let Some(target) = depth.checked_sub(1) else {
            return;
        };

        while trunk.get(j).depth() != target {
            *i -= 1;
            j -= 1;
        }
    }

    pub fn up_down(&mut self, i: &mut usize, delta: isize) {
        if let Some(next) = i.checked_add_signed(delta) {
            if next < self.len() {
                *i = next;
            }
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
