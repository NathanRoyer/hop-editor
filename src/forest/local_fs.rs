use super::*;

fn pop_dir_slash(text: &str) -> &str {
    let dec = text.ends_with('/') as usize;
    let base_len = text.len() - dec;
    &text[..base_len]
}

pub struct FsTrunk {
    id: Arc<str>,
    prefix: String,
    entries: Vec<Entry>,
    walker: Walker,
}

#[derive(PartialEq, Eq)]
struct Entry {
    name: String,
    depth: usize,
}

impl EntryApi for Entry {
    fn name(&self) -> &str {
        &self.name
    }

    fn depth(&self) -> usize {
        self.depth
    }

    fn is_dir(&self) -> bool {
        self.name.ends_with('/')
    }
}

impl FsTrunk {
    pub fn new(mut path: &str) -> Self {
        while let Some(prev) = path.strip_suffix("/") {
            path = prev;
        }

        let id = Arc::from(path);

        let (mut prefix, mut name) = match path.rsplit_once('/') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (".".to_string(), path.to_string()),
        };

        prefix.push('/');
        name.push('/');

        let base = Entry {
            name,
            depth: 0,
        };

        Self {
            id,
            prefix,
            entries: vec![base],
            walker: Walker::default(),
        }
    }

    fn insert_entry(&mut self, mut i: usize, name: String) {
        use cmp::Ordering::{Less, Equal};
        let parent = &self.entries[i];
        let inc_depth = parent.depth + 1;

        let entry = Entry {
            name,
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

    // note: should not use path walker here
    fn delete(&mut self, i: usize) -> io::Result<()> {
        let old_path = self.walker.result();
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

    fn try_act(&mut self, i: usize, action: MenuItem) -> io::Result<()> {
        use MenuItem::*;

        let old_path = self.walker.result();
        let mut new_path = old_path.to_string();
        let entry = &mut self.entries[i];
        assert!(old_path.ends_with(entry.name()));
        let old_name = pop_dir_slash(entry.name());

        match action {
            Rename => {
                let Some(mut new_name) = prompt!("New name for {old_name:?}:") else {
                    return Ok(());
                };

                utils::replace_last(&mut new_path, &old_name, &new_name);
                fs::rename(old_path, new_path)?;

                if entry.is_dir() {
                    new_name.push('/');
                }
                entry.name = new_name;
            },
            Delete => self.delete(i)?,
            NewDir => {
                let Some(mut dir_name) = prompt!("Name of new directory in {old_name:?}:") else {
                    return Ok(());
                };

                dir_name.push('/');
                new_path += &dir_name;
                fs::create_dir(new_path)?;
                self.insert_entry(i, dir_name);
            },
            NewFile => {
                let Some(file_name) = prompt!("Name of new file in {old_name:?}:") else {
                    return Ok(());
                };

                new_path += &file_name;
                fs::write(new_path, "")?;
                self.insert_entry(i, file_name);
            },
            other => _ = alert!("Bad Code Path ({other:?})"),
        }

        Ok(())
    }

    fn prepare_path(&mut self, i: usize) {
        let mut walker = take(&mut self.walker);
        walker.walk(self, i);
        self.walker = walker;
    }
}

impl AnchorApi for FsTrunk {
    fn prefix(&self) -> &str {
        &self.prefix
    }
}

impl TrunkApi for FsTrunk {
    fn id(&self) -> TrunkId {
        self.id.clone()
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, i: usize) -> &dyn EntryApi {
        &self.entries[i]
    }

    fn file_key(&mut self, i: usize) -> FileKey {
        self.prepare_path(i);
        let path = self.walker.result().to_string();
        let trunk = self.id();
        FileKey::new(trunk, path)
    }

    fn file_text(&mut self, path: &str) -> Result<String, String> {
        fs::read_to_string(path).map_err(|e| format!("{e}"))
    }

    fn save_file(&mut self, path: &str, text: &str) -> Result<(), String> {
        save(path, text)
    }

    fn open_dir(&mut self, i: usize) {
        self.prepare_path(i);
        let inc_depth = self.entries[i].depth + 1;
        let path = self.walker.result();

        if self.is_dir_open(i) {
            return;
        }

        let j = i + 1;
        let suffix = self.entries.split_off(j);

        if let Err(error) = read_dir(path, &mut self.entries, inc_depth) {
            alert!("failed to read directory: {error:?}");
            self.entries.truncate(j);
        };

        self.entries[j..].sort();
        self.entries.extend(suffix);
    }

    fn close_dir(&mut self, i: usize) {
        let orig_depth = self.entries[i].depth;
        let suffix = self.entries.split_off(i + 1);
        let crit = |e: &Entry| e.depth > orig_depth;
        self.entries.extend(suffix.into_iter().skip_while(crit));
    }

    fn menu(&mut self, i: usize, options: Options) {
        use MenuItem::*;

        if self.get(i).is_dir() {
            options.extend([NewFile, NewDir]);
        }

        options.extend([Rename, Delete]);
    }

    fn act(&mut self, i: usize, action: MenuItem) {
        if let Err(error) = self.try_act(i, action) {
            alert!("Failure: {error:?}");
        }
    }

    fn reveal(&mut self, path: &str) -> Option<usize> {
        utils::reveal(self, path)
    }
}

fn read_dir(dir_path: &str, entries: &mut Vec<Entry>, depth: usize) -> io::Result<()> {
    let mut empty = true;

    for item in fs::read_dir(dir_path)? {
        let Ok(item) = item else {
            continue;
        };

        let Ok(mut name) = item.file_name().into_string() else {
            continue;
        };

        let hidden = hide_folder(&name);

        let (Ok(ft), false) = (item.file_type(), hidden) else {
            continue;
        };

        if ft.is_dir() {
            name.push('/');
        }

        let entry = Entry {
            name,
            depth,
        };

        entries.push(entry);
        empty = false;
    }

    if empty {
        let entry = Entry {
            name: String::from("<empty>"),
            depth,
        };

        entries.push(entry);
    }

    Ok(())
}

pub fn save(path: &str, text: &str) -> Result<(), String> {
    fs::write(path, text).map_err(|e| format!("{e}"))
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
