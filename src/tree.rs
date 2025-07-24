use litemap::LiteMap;
use crate::tab::TabKey;

pub type EntryMap = LiteMap<String, Entry>;

pub enum Entry {
    // none => folded
    // some => unfolded
    Directory(Option<EntryMap>),

    // none => closed
    // some => opened in a tab
    File(Option<TabKey>),
}

pub struct Root {
    path: String,

    // none => folded
    // some => unfolded
    dir: Option<EntryMap>,
}
