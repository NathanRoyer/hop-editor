use enum_dispatch::enum_dispatch;
use super::*;

pub type Options<'a> = &'a mut Vec<MenuItem>;
pub type TrunkId = Arc<str>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FileKey {
    trunk: Option<TrunkId>,
    path: String,
}

impl FileKey {
    pub fn new(trunk: TrunkId, path: String) -> Self {
        Self { trunk: Some(trunk), path }
    }

    pub fn fallback(path: String) -> Self {
        Self { trunk: None, path }
    }

    pub fn trunk(&self) -> Option<&str> {
        self.trunk.as_deref()
    }

    pub fn path(&self) -> &str {
        &self.path
    }
}

pub trait EntryApi {
    fn name(&self) -> &str;
    fn depth(&self) -> usize;
    fn is_dir(&self) -> bool;
}

#[enum_dispatch(Trunk)]
pub trait TrunkApi {
    fn id(&self) -> TrunkId;

    fn len(&self) -> usize;
    fn get(&self, i: usize) -> &dyn EntryApi;

    fn file_key(&mut self, i: usize) -> FileKey;
    fn file_text(&mut self, path: &str) -> Result<String, String>;
    fn save_file(&mut self, path: &str, text: &str) -> Result<(), String>;

    fn search(&mut self, i: usize, text: &str) -> Vec<String> {
        Vec::new()
    }

    fn open_dir(&mut self, i: usize);
    fn close_dir(&mut self, i: usize);

    fn menu(&mut self, i: usize, options: Options) {}
    fn act(&mut self, i: usize, action: MenuItem) {}

    fn reveal(&mut self, path: &str) -> Option<usize> {
        None
    }

    // extension for search_fs
    fn search_term(&self) -> Option<String> {
        None
    }

    fn is_dir_open(&self, mut i: usize) -> bool {
        let depth = self.get(i).depth();
        i += 1;

        match i < self.len() {
            true => self.get(i).depth() > depth,
            false => false,
        }
    }
}

pub trait AnchorApi: TrunkApi {
    fn prefix(&self) -> &str;
}

#[enum_dispatch]
pub enum Trunk {
    FsTrunk,
    SearchTrunk,
}
