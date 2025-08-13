#![allow(unused_variables)]

use std::{io, fs, cmp};
use std::fmt::Write;
use std::sync::Arc;
use std::mem::take;

use crate::interface::menu::{MenuItem, context_menu};
use crate::{alert, confirm, prompt};
use crate::config::hide_folder;

pub use entry::FileKey;

use entry::{Options, EntryApi, TrunkApi, AnchorApi, TrunkId};
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

    pub fn add_local_folder(&mut self, path: &str) -> TrunkId {
        let trunk = FsTrunk::new(path);
        let id = trunk.id();
        self.trunks.push(trunk);
        id
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
        Some(index as usize + self.scroll)
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

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn trunk_by_id(&mut self, trunk_id: &str) -> Option<&mut FsTrunk> {
        self
            .trunks
            .iter_mut()
            .find(|t| &*t.id() == trunk_id)
    }

    pub fn reveal(&mut self, key: &FileKey) -> Option<usize> {
        let id = key.trunk()?;
        let trunk = self.trunk_by_id(id)?;
        trunk.reveal(&key.path())
    }

    pub fn click_line(&mut self, line: usize) -> Option<(FileKey, String)> {
        self.click_index(line + self.scroll)
    }

    pub fn click_index(&mut self, mut i: usize) -> Option<(FileKey, String)> {
        let trunk = self.trunk_mut(&mut i)?;

        if trunk.get(i).is_dir() {
            match trunk.is_dir_open(i) {
                true => trunk.close_dir(i),
                false => trunk.open_dir(i),
            }

            None
        } else {
            let key = trunk.file_key(i);
            self.open(&key).map(|text| (key, text))
        }
    }

    pub fn open(&mut self, key: &FileKey) -> Option<String> {
        let id = key.trunk()?;
        let trunk = self.trunk_by_id(id)?;

        match trunk.file_text(key.path()) {
            Ok(text) => Some(text),
            Err(error) => {
                alert!("{}: {error}", key.path());
                None
            },
        }
    }

    pub fn save(&mut self, key: &FileKey, text: &str) -> Result<(), ()> {
        let result = match key.trunk() {
            Some(id) => match self.trunk_by_id(id) {
                Some(trunk) => trunk.save_file(key.path(), text),
                None => Err("Failed to find file trunk".to_string()),
            },
            None => local_fs::save(key.path(), text),
        };

        if let Err(error) = result {
            alert!("{}: {error}", key.path());
            Err(())
        } else {
            Ok(())
        }
    }

    pub fn right_click<F: Fn(&FileKey) -> bool>(
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
        trunk.menu(i, &mut options);

        if let Some(action) = context_menu(x, y, &options) {
            let forbid_use = [MenuItem::Rename, MenuItem::Delete];
            let key = trunk.file_key(i);
            let in_use = is_in_use(&key);

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
