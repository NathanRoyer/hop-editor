use super::*;

pub type Options<'a> = &'a mut Vec<MenuItem>;

pub trait EntryApi {
    fn name(&self) -> &str;
    fn depth(&self) -> usize;
    fn is_dir(&self) -> bool;
}

pub trait TrunkApi {
    fn prefix(&self) -> &str;
    fn len(&self) -> usize;
    fn get(&self, i: usize) -> &dyn EntryApi;

    fn open_file(&mut self, i: usize) -> String;

    fn open_dir(&mut self, i: usize);
    fn close_dir(&mut self, i: usize);

    fn menu(&mut self, i: usize, options: Options);
    fn act(&mut self, i: usize, action: MenuItem);

    fn is_dir_open(&self, mut i: usize) -> bool {
        let depth = self.get(i).depth();
        i += 1;

        match i < self.len() {
            true => self.get(i).depth() > depth,
            false => false,
        }
    }
}
