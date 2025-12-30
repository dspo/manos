#[derive(Clone, Debug)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) orig_path: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct CommitEntry {
    pub(crate) hash: String,
    pub(crate) short_hash: String,
    pub(crate) subject: String,
}
