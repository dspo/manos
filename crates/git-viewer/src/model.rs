#[derive(Clone, Debug)]
pub(crate) struct FileEntry {
    pub(crate) path: String,
    pub(crate) status: String,
    pub(crate) orig_path: Option<String>,
}
