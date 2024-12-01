use tabled::Tabled;

#[derive(PartialEq, Tabled)]
pub struct Cleared {
    pub(crate) Program: String,
}
impl PartialEq<Option<Cleared>> for &Cleared {
    fn eq(&self, other: &Option<Cleared>) -> bool {
        match other {
            Some(other) => other.Program.eq(&*self.Program),
            None => false,
        }
    }
}
#[derive(Clone)]
pub struct CleanerData {
    pub path: String,
    pub category: String,
    pub program: String,

    pub files_to_remove: Vec<String>,
    pub folders_to_remove: Vec<String>,
    pub directories_to_remove: Vec<String>,

    pub remove_all_in_dir: bool,
    pub remove_directory_after_clean: bool,
    pub remove_directories: bool,
    pub remove_files: bool
}
pub struct CleanerResult {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub working: bool,
    pub path: String,
    pub program: String
}