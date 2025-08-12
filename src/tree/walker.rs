use super::*;

#[derive(Default)]
pub struct PathWalker {
    buf: String,
    path_i: Vec<usize>,
}

impl PathWalker {
    pub fn walk<'a, T: TrunkApi>(&'a mut self, trunk: &T, i: usize) -> &'a str {
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
}
