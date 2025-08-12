#![allow(unused_variables)]

use std::{io, fs, cmp};
use std::fmt::Write;
use std::mem::take;

use crate::interface::menu::{MenuItem, context_menu};
use crate::{alert, confirm, prompt};
use crate::config::hide_folder;

use entry::{Options, EntryApi, TrunkApi};
use local_fs::FsTrunk;
use utils::Walker;

mod entry;
mod utils;
mod local_fs;

// syms: ▷▽▶▼;

pub struct Forest {
    trunks: Vec<FsTrunk>,
    scroll: usize,
}

impl Forest {
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

    pub fn reveal_path(&mut self, path: &str) -> Option<usize> {
        self.trunks.iter_mut().find_map(|t| t.reveal(path))
    }

    pub fn click_line(&mut self, line: usize) -> Option<&str> {
        self.click_index(line + self.scroll)
    }

    pub fn click_index(&mut self, mut i: usize) -> Option<&str> {
        let trunk = self.trunk_mut(&mut i)?;

        if trunk.get(i).is_dir() {
            match trunk.is_dir_open(i) {
                true => trunk.close_dir(i),
                false => trunk.open_dir(i),
            }

            None
        } else {
            trunk.prepare_path(i);
            Some(trunk.get_path())
        }
    }

    pub fn right_click<F: Fn(&str) -> bool>(
        &mut self,
        x: u16,
        y: u16,
        entry: u16,
        is_in_use: F,
    ) {
        let mut i = entry as usize + self.scroll;
        let Some(trunk) = self.trunk_mut(&mut i) else {
            return;
        };

        let mut options = Vec::with_capacity(8);
        trunk.prepare_path(i);
        trunk.menu(i, &mut options);
        let path = trunk.get_path();

        if let Some(action) = context_menu(x, y, &options) {
            let forbid_use = [MenuItem::Rename, MenuItem::Delete];
            let in_use = is_in_use(&path);

            if forbid_use.contains(&action) && in_use {
                alert!("Not possible!\nAt least one of your tabs relies on this path.");
                return;
            }

            trunk.act(i, action);
        }
    }

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
