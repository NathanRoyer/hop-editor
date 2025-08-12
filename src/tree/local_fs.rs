use super::*;

fn pop_dir_slash(text: &str) -> &str {
    let dec = text.ends_with('/') as usize;
    let base_len = text.len() - dec;
    &text[..base_len]
}

pub struct FsTrunk {
    prefix: String,
    entries: Vec<Entry>,
    walker: PathWalker,
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
    pub fn new(mut path: String) -> Self {
        while path.ends_with("/") {
            path.pop();
        }

        let (mut prefix, mut name) = match path.rsplit_once('/') {
            Some((a, b)) => (a.to_string(), b.to_string()),
            None => (".".to_string(), path),
        };

        prefix.push('/');
        name.push('/');

        let base = Entry {
            name,
            depth: 0,
        };

        Self {
            prefix,
            entries: vec![base],
            walker: PathWalker::default(),
        }
    }
}

impl TrunkApi for FsTrunk {
    fn prefix(&self) -> &str {
        &self.prefix
    }

    fn len(&self) -> usize {
        self.entries.len()
    }

    fn get(&self, i: usize) -> &dyn EntryApi {
        &self.entries[i]
    }

    fn open_file(&mut self, i: usize) -> String {
        let mut walker = take(&mut self.walker);
        let path = walker.walk(self, i).to_string();
        self.walker = walker;
        path
    }

    fn open_dir(&mut self, i: usize) {
        let inc_depth = self.entries[i].depth + 1;
        let mut walker = take(&mut self.walker);
        let path = walker.walk(self, i);

        if self.is_dir_open(i) {
            self.walker = walker;
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

        self.walker = walker;
    }

    fn close_dir(&mut self, i: usize) {
        let orig_depth = self.entries[i].depth;
        let suffix = self.entries.split_off(i + 1);
        let crit = |e: &Entry| e.depth > orig_depth;
        self.entries.extend(suffix.into_iter().skip_while(crit));
    }

    fn menu(&mut self, i: usize, options: Options) {
        todo!();
    }

    fn act(&mut self, i: usize, action: MenuItem) {
        todo!();
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
