use super::*;

pub struct SearchTrunk {
    results: Vec<SearchResult>,
    searched: String,
    folded: bool,
    id: TrunkId,
}

impl SearchTrunk {
    pub fn new(id: TrunkId, paths: Vec<String>, searched: String) -> Self {
        let results = paths
            .into_iter()
            .map(|p| FileKey::new(id.clone(), p))
            .map(|k| SearchResult(k))
            .collect();

        Self {
            results,
            searched,
            folded: false,
            id,
        }
    }
}

pub struct SearchResult(FileKey);

impl EntryApi for SearchResult {
    fn name(&self) -> &str {
        match self.0.path().rsplit_once('/') {
            Some((_, name)) => name,
            None => self.0.path(),
        }
    }

    fn depth(&self) -> usize { 1 }
    fn is_dir(&self) -> bool { false }
}

impl EntryApi for SearchTrunk {
    fn name(&self) -> &str {
        "[Search Results]"
    }

    fn depth(&self) -> usize { 0 }
    fn is_dir(&self) -> bool { true }
}

impl TrunkApi for SearchTrunk {
    fn id(&self) -> TrunkId {
        self.id.clone()
    }

    fn len(&self) -> usize {
        match self.folded {
            true => 1,
            false => self.results.len() + 1,
        }
    }

    fn get(&self, i: usize) -> &dyn EntryApi {
        match i.checked_sub(1) {
            Some(i) => &self.results[i],
            None => self,
        }
    }

    fn file_key(&mut self, i: usize) -> FileKey {
        self.results[i.saturating_sub(1)].0.clone()
    }

    fn file_text(&mut self, _path: &str) -> Result<String, String> {
        unreachable!();
    }

    fn save_file(&mut self, _path: &str, _text: &str) -> Result<(), String> {
        unreachable!()
    }

    fn open_dir(&mut self, _i: usize) {
        self.folded = false;
    }

    fn close_dir(&mut self, _i: usize) {
        self.folded = true;
    }

    fn search_term(&self) -> Option<String> {
        Some(self.searched.clone())
    }
}
