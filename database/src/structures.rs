use bitflags::bitflags;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

// INFO: Struct for GUI table (tabled removed - CLI table not used)
#[derive(PartialEq, Clone)]
pub struct Cleared {
    pub program: String,
    pub removed_bytes: u64,
    pub removed_files: u64,
    pub removed_directories: u64,
    pub affected_categories: Vec<String>,
}

impl PartialEq<Option<Cleared>> for &Cleared {
    fn eq(&self, other: &Option<Cleared>) -> bool {
        match other {
            Some(other) => other.program.eq(&*self.program),
            None => false,
        }
    }
}

bitflags! {
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Default, Serialize, Deserialize)]
    #[serde(transparent)]
    pub struct CleanerFlags: u8 {
        const REMOVE_ALL_IN_DIR              = 1 << 0;
        const REMOVE_DIRECTORY_AFTER_CLEAN   = 1 << 1;
        const REMOVE_DIRECTORIES             = 1 << 2;
        const REMOVE_FILES                   = 1 << 3;
    }
}

// helper for legacy bool/u8 fields
#[derive(Deserialize)]
#[serde(untagged)]
enum BoolOrU8 {
    Bool(bool),
    U8(u8),
}

impl BoolOrU8 {
    fn as_u8(&self) -> u8 {
        match self {
            BoolOrU8::Bool(b) => *b as u8,
            BoolOrU8::U8(n) => *n,
        }
    }
}

// INFO: Struct for clearing files and folders
#[derive(Clone)]
pub struct CleanerData {
    pub path: String,
    pub category: String,
    pub program: String,
    pub class: String,
    pub sub_category: String,
    pub files_to_remove: Vec<String>,
    pub directories_to_remove: Vec<String>,
    pub flags: CleanerFlags,
}

impl CleanerData {
    pub fn remove_all_in_dir(&self) -> bool {
        self.flags.contains(CleanerFlags::REMOVE_ALL_IN_DIR)
    }
    pub fn remove_directory_after_clean(&self) -> bool {
        self.flags.contains(CleanerFlags::REMOVE_DIRECTORY_AFTER_CLEAN)
    }
    pub fn remove_directories(&self) -> bool {
        self.flags.contains(CleanerFlags::REMOVE_DIRECTORIES)
    }
    pub fn remove_files(&self) -> bool {
        self.flags.contains(CleanerFlags::REMOVE_FILES)
    }
    pub fn set_remove_all_in_dir(&mut self, v: bool) {
        self.flags
            .set(CleanerFlags::REMOVE_ALL_IN_DIR, v);
    }
    pub fn set_remove_directory_after_clean(&mut self, v: bool) {
        self.flags
            .set(CleanerFlags::REMOVE_DIRECTORY_AFTER_CLEAN, v);
    }
    pub fn set_remove_directories(&mut self, v: bool) {
        self.flags.set(CleanerFlags::REMOVE_DIRECTORIES, v);
    }
    pub fn set_remove_files(&mut self, v: bool) {
        self.flags.set(CleanerFlags::REMOVE_FILES, v);
    }
}

// custom Deserialize to keep backward compat with old JSON bool fields
impl<'de> Deserialize<'de> for CleanerData {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Helper {
            path: String,
            category: String,
            program: String,
            #[serde(default = "default_class")]
            class: String,
            #[serde(default, alias = "sub_class", alias = "subCategory")]
            sub_category: String,
            #[serde(default)]
            files_to_remove: Vec<String>,
            #[serde(default)]
            directories_to_remove: Vec<String>,
            // new byte flag field
            #[serde(default)]
            flags: Option<u8>,
            // legacy bool fields: accept bool or u8 (byte flag per-field)
            #[serde(default)]
            remove_all_in_dir: Option<BoolOrU8>,
            #[serde(default)]
            remove_directory_after_clean: Option<BoolOrU8>,
            #[serde(default)]
            remove_directories: Option<BoolOrU8>,
            #[serde(default)]
            remove_files: Option<BoolOrU8>,
        }

        let h = Helper::deserialize(deserializer)?;
        let mut bits: u8 = h.flags.unwrap_or(0);
        if let Some(v) = h.remove_all_in_dir {
            if v.as_u8() != 0 {
                bits |= CleanerFlags::REMOVE_ALL_IN_DIR.bits();
            }
        }
        if let Some(v) = h.remove_directory_after_clean {
            if v.as_u8() != 0 {
                bits |= CleanerFlags::REMOVE_DIRECTORY_AFTER_CLEAN.bits();
            }
        }
        if let Some(v) = h.remove_directories {
            if v.as_u8() != 0 {
                bits |= CleanerFlags::REMOVE_DIRECTORIES.bits();
            }
        }
        if let Some(v) = h.remove_files {
            if v.as_u8() != 0 {
                bits |= CleanerFlags::REMOVE_FILES.bits();
            }
        }

        Ok(CleanerData {
            path: h.path,
            category: h.category,
            program: h.program,
            class: h.class,
            sub_category: h.sub_category,
            files_to_remove: h.files_to_remove,
            directories_to_remove: h.directories_to_remove,
            flags: CleanerFlags::from_bits_truncate(bits),
        })
    }
}

// Serialize emits both new `flags` and legacy bool fields for max compat
impl CleanerData {
    fn serialize_legacy<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;
        let mut s = serializer.serialize_struct("CleanerData", 9)?;
        s.serialize_field("path", &self.path)?;
        s.serialize_field("category", &self.category)?;
        s.serialize_field("program", &self.program)?;
        s.serialize_field("class", &self.class)?;
        s.serialize_field("sub_category", &self.sub_category)?;
        s.serialize_field("files_to_remove", &self.files_to_remove)?;
        s.serialize_field("directories_to_remove", &self.directories_to_remove)?;
        s.serialize_field("flags", &self.flags.bits())?;
        // legacy aliases so old readers still work if they ignore `flags`
        s.serialize_field(
            "remove_all_in_dir",
            &self.flags.contains(CleanerFlags::REMOVE_ALL_IN_DIR),
        )?;
        s.serialize_field(
            "remove_directory_after_clean",
            &self
                .flags
                .contains(CleanerFlags::REMOVE_DIRECTORY_AFTER_CLEAN),
        )?;
        s.serialize_field(
            "remove_directories",
            &self.flags.contains(CleanerFlags::REMOVE_DIRECTORIES),
        )?;
        s.serialize_field(
            "remove_files",
            &self.flags.contains(CleanerFlags::REMOVE_FILES),
        )?;
        s.end()
    }
}

// delegate Serialize to legacy-compatible impl
impl Serialize for CleanerData {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.serialize_legacy(serializer)
    }
}

// INFO: Struct for clearing registry
// WARN: Windows only
#[cfg(windows)]
#[derive(Serialize, Deserialize, Clone)]
pub struct CleanerDataRegistry {
    pub category: String,
    pub program: String,
    #[serde(default = "default_class")]
    pub class: String,
    #[serde(default, alias = "sub_class")]
    pub sub_category: String,

    #[serde(default)]
    pub remove_all_in_tree: bool,
    #[serde(default)]
    pub remove_all_in_registry: bool,

    #[serde(default)]
    pub path: String,

    #[serde(default)]
    pub values_to_remove: Vec<String>,

    #[serde(default)]
    pub keys_to_remove: Vec<String>,

    // INFO: Glob matched against the last segment(s) of resolved paths:
    // value names for remove_values, subkey names for remove_trees.
    // "true" = remove all values / the resolved key tree itself
    #[serde(default)]
    pub remove_values: String,

    #[serde(default)]
    pub remove_trees: String,
}

// INFO: Lightweight projection of CleanerData used for category scans. Unknown
// fields (files_to_remove, directories_to_remove, flags, class, ...) are ignored
// by serde and never allocated. `path` is kept only to detect the {drive}
// placeholder so entry counts stay identical to the full database.
#[derive(Deserialize, Clone)]
pub struct CleanerIndex {
    #[serde(default)]
    pub path: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub program: String,
    #[serde(default, alias = "sub_class", alias = "subCategory")]
    pub sub_category: String,
}

impl From<&CleanerData> for CleanerIndex {
    fn from(data: &CleanerData) -> Self {
        Self {
            path: data.path.clone(),
            category: data.category.clone(),
            program: data.program.clone(),
            sub_category: data.sub_category.clone(),
        }
    }
}

// INFO: Lightweight projection of CleanerDataRegistry for category scans.
// WARN: Windows only
#[cfg(windows)]
#[derive(Deserialize, Clone)]
pub struct RegistryIndex {
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub program: String,
    #[serde(default, alias = "sub_class")]
    pub sub_category: String,
}

#[cfg(windows)]
impl From<&CleanerDataRegistry> for RegistryIndex {
    fn from(data: &CleanerDataRegistry) -> Self {
        Self {
            category: data.category.clone(),
            program: data.program.clone(),
            sub_category: data.sub_category.clone(),
        }
    }
}

fn default_class() -> String {
    String::from("Other")
}

// INFO: Built-in custom cleaning (defined in code, not JSON).
// Each entry describes: category, sub_category, target OS and the cleaning
// function itself. Functions are registered at runtime via
// database::custom_cleaners::register_custom_cleaner (see cleaner::custom_cleaners::register_all).
pub type CustomCleanFn = fn(
    &CustomCleaner,
    Option<tokio::sync::mpsc::Sender<String>>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = CleanerResult> + Send>>;

#[derive(Clone)]
pub struct CustomCleaner {
    /// Unique id, used to enable/disable this cleaner from CLI/GUI
    pub id: String,
    pub program: String,
    pub category: String,
    pub sub_category: String,
    /// Target file or directory. Supports {username} placeholder
    pub path: String,
    /// Extra arguments passed to the cleaning function
    pub args: Vec<String>,
    /// Operating systems this cleaning applies to (empty = all)
    /// Values: "windows", "linux", "macos"
    pub os: Vec<String>,
    /// The cleaning function that performs the custom cleanup
    pub function: CustomCleanFn,
    /// If true, this cleaner runs sequentially (not concurrent with others)
    /// so that its progress messages don't mix with other cleaners.
    pub sequential: bool,
}

impl CustomCleaner {
    pub fn matches_current_os(&self) -> bool {
        if self.os.is_empty() {
            return true;
        }
        let current = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "linux") {
            "linux"
        } else {
            "macos"
        };
        self.os.iter().any(|os| os.eq_ignore_ascii_case(current))
    }
}

// INFO: Struct for task clearing (Result cleared)
pub struct CleanerResult {
    pub files: u64,
    pub folders: u64,
    pub bytes: u64,
    pub working: bool,
    pub path: String,
    pub program: String,
    pub category: String,
    pub sub_category: String,
}
