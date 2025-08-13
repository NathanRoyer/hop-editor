use super::*;

#[derive(Default)]
pub struct Walker {
    buf: String,
    path_i: Vec<usize>,
}

impl Walker {
    pub fn walk<'a, T: AnchorApi>(&'a mut self, trunk: &T, i: usize) -> &'a str {
        let mut entry = trunk.get(i);
        let mut depth = entry.depth();
        let mut j = i;

        self.path_i.push(i);

        while j > 0 {
            j -= 1;
            entry = trunk.get(j);

            if entry.is_dir() && entry.depth() < depth {
                self.path_i.push(j);
                depth = entry.depth();
            }
        }

        self.buf.clear();
        self.buf += trunk.prefix();

        for i in self.path_i.drain(..).rev() {
            self.buf += trunk.get(i).name();
        }

        &self.buf
    }

    pub fn result(&self) -> &str {
        &self.buf
    }
}

pub fn reveal<T: AnchorApi>(trunk: &mut T, mut path: &str) -> Option<usize> {
    path = path.strip_prefix(trunk.prefix())?;
    let mut i = 0;

    while i < trunk.len() {
        let entry = trunk.get(i);

        if let Some(next) = path.strip_prefix(&entry.name()) {
            path = next;

            if path.is_empty() {
                return Some(i);
            }

            if entry.is_dir() {
                trunk.open_dir(i);
            }
        }

        i += 1;
    }

    None
}

pub fn replace_last(dst: &mut String, from: &str, to: &str) {
    let start = dst
        .rfind(from)
        .expect("replace_last: no occurence!");

    let stop = start + from.len();
    dst.replace_range(start..stop, to);
}
