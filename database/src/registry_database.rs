#[cfg(windows)]
use crate::registry_utils::{
    expand_registry_path_pattern, remove_all_in_registry, remove_all_in_tree_in_registry,
    remove_key_in_registry, remove_trees_matching_in_registry, remove_values_matching_in_registry,
};
#[cfg(windows)]
use crate::streaming::for_each_array;
#[cfg(windows)]
use crate::structures::CleanerDataRegistry;
#[cfg(windows)]
use crate::structures::CleanerResult;
#[cfg(windows)]
use crate::structures::RegistryIndex;
#[cfg(windows)]
use flate2::read::GzDecoder;
#[cfg(windows)]
use std::error::Error;
#[cfg(windows)]
use std::fs::File;
#[cfg(windows)]
use std::io::BufReader;
#[cfg(windows)]
use std::path::PathBuf;
#[cfg(windows)]
use std::sync::Arc;
#[cfg(windows)]
use std::sync::OnceLock;
#[cfg(windows)]
use winreg::RegKey;
#[cfg(windows)]
use winreg::enums::*;

/// Where the registry database is read from.
#[cfg(windows)]
#[derive(Clone)]
enum RegistrySource {
    /// Built-in database compiled into the binary (minified + gzip).
    Default,
    /// Database file supplied on the command line (plain JSON array).
    File(PathBuf),
    /// In-memory database (tests).
    Memory(Arc<[CleanerDataRegistry]>),
}

/// Lazy handle over the registry database. Stores only the source; entries are
/// streamed on demand via [`RegistryDatabase::for_each`].
#[cfg(windows)]
#[derive(Clone)]
pub struct RegistryDatabase {
    source: RegistrySource,
}

#[cfg(windows)]
impl RegistryDatabase {
    pub fn default_source() -> Self {
        Self {
            source: RegistrySource::Default,
        }
    }

    pub fn from_file<P: Into<PathBuf>>(path: P) -> Self {
        Self {
            source: RegistrySource::File(path.into()),
        }
    }

    pub fn from_vec(entries: Vec<CleanerDataRegistry>) -> Self {
        Self {
            source: RegistrySource::Memory(entries.into()),
        }
    }

    /// Stream every entry through `f`. Re-reads and re-decompresses the source
    /// on each call; only one entry is alive at a time.
    pub fn for_each<F>(&self, mut f: F) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(CleanerDataRegistry),
    {
        match &self.source {
            RegistrySource::Default => {
                let compressed_data =
                    include_bytes!(concat!(env!("OUT_DIR"), "/registry_database.min.json.gz"));
                let decoder = GzDecoder::new(&compressed_data[..]);
                for_each_array(decoder, &mut f)?;
            }
            RegistrySource::File(path) => {
                let reader = BufReader::new(File::open(path)?);
                for_each_array(reader, &mut f)?;
            }
            RegistrySource::Memory(entries) => {
                for entry in entries.iter().cloned() {
                    f(entry);
                }
            }
        }

        Ok(())
    }

    /// Stream lightweight index entries ([`RegistryIndex`]); only category,
    /// program and sub_category are deserialized.
    pub fn for_each_index<F>(&self, mut f: F) -> Result<(), Box<dyn Error>>
    where
        F: FnMut(RegistryIndex),
    {
        match &self.source {
            RegistrySource::Default => {
                let compressed_data =
                    include_bytes!(concat!(env!("OUT_DIR"), "/registry_database.min.json.gz"));
                let decoder = GzDecoder::new(&compressed_data[..]);
                for_each_array(decoder, &mut f)?;
            }
            RegistrySource::File(path) => {
                let reader = BufReader::new(File::open(path)?);
                for_each_array(reader, &mut f)?;
            }
            RegistrySource::Memory(entries) => {
                for entry in entries.iter() {
                    f(RegistryIndex::from(entry));
                }
            }
        }

        Ok(())
    }
}

#[cfg(windows)]
static DATABASE: OnceLock<Arc<[CleanerDataRegistry]>> = OnceLock::new();

#[cfg(windows)]
pub fn get_default_database() -> Arc<[CleanerDataRegistry]> {
    DATABASE
        .get_or_init(|| {
            let mut entries = Vec::new();
            RegistryDatabase::default_source()
                .for_each(|entry| entries.push(entry))
                .expect("Failed to parse database");
            entries.into()
        })
        .clone()
}

#[cfg(windows)]
pub fn get_database_from_file(
    file_path: &str,
) -> Result<Arc<[CleanerDataRegistry]>, Box<dyn Error>> {
    let mut entries = Vec::new();
    RegistryDatabase::from_file(file_path).for_each(|entry| entries.push(entry))?;
    Ok(entries.into())
}

#[cfg(windows)]
pub fn clear_registry(data: &CleanerDataRegistry) -> CleanerResult {
    // INFO: Temporary clearing bytes result
    let mut removed: u64 = 0;

    // INFO: Creating output struct
    let mut result = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        path: data.path.clone(),
        program: data.program.clone(),
        category: data.category.clone(),
        sub_category: data.sub_category.clone(),
    };

    // INFO: Parsing registry key
    let root = if data.path.starts_with("HKEY_CURRENT_USER") {
        Some(RegKey::predef(HKEY_CURRENT_USER))
    } else if data.path.starts_with("HKEY_LOCAL_MACHINE") {
        Some(RegKey::predef(HKEY_LOCAL_MACHINE))
    } else if data.path.starts_with("HKEY_CLASSES_ROOT") {
        Some(RegKey::predef(HKEY_CLASSES_ROOT))
    } else if data.path.starts_with("HKEY_USERS") {
        Some(RegKey::predef(HKEY_USERS))
    } else if data.path.starts_with("HKEY_CURRENT_CONFIG") {
        Some(RegKey::predef(HKEY_CURRENT_CONFIG))
    } else {
        None
    };

    // INFO: Removing registry key from path
    let path = if data.path.starts_with("HKEY_CURRENT_USER\\") {
        Some(data.path.replace("HKEY_CURRENT_USER\\", ""))
    } else if data.path.starts_with("HKEY_LOCAL_MACHINE\\") {
        Some(data.path.replace("HKEY_LOCAL_MACHINE\\", ""))
    } else if data.path.starts_with("HKEY_CLASSES_ROOT\\") {
        Some(data.path.replace("HKEY_CLASSES_ROOT\\", ""))
    } else if data.path.starts_with("HKEY_USERS\\") {
        Some(data.path.replace("HKEY_USERS\\", ""))
    } else if data.path.starts_with("HKEY_CURRENT_CONFIG\\") {
        Some(data.path.replace("HKEY_CURRENT_CONFIG\\", ""))
    } else {
        None
    };

    // INFO: Main logic
    if let (Some(root), Some(path)) = (root, path) {
        // INFO: Expand glob pattern in main path ("*" and "?" per segment)
        let paths: Vec<String> = if path.contains('*') || path.contains('?') {
            expand_registry_path_pattern(&root, &path)
        } else {
            vec![path.clone()]
        };

        for current_path in paths {
            if data.remove_all_in_tree {
                removed += remove_all_in_tree_in_registry(&root, current_path.clone())
            }
            if data.remove_all_in_registry {
                removed += remove_all_in_registry(&root, current_path.clone())
            }
            // INFO: remove_values is the glob for value names matched at
            // the end of resolved paths, "true" removes all values
            if data.remove_values == "true" {
                removed += remove_all_in_registry(&root, current_path.clone())
            } else if !data.remove_values.is_empty() {
                removed += remove_values_matching_in_registry(
                    &root,
                    current_path.clone(),
                    data.remove_values.clone(),
                )
            }
            // INFO: remove_trees is the glob for subkey trees matched at
            // the end of resolved paths, "true" removes resolved keys
            if data.remove_trees == "true" {
                removed += remove_key_in_registry(&root, current_path.clone())
            } else if !data.remove_trees.is_empty() {
                removed += remove_trees_matching_in_registry(
                    &root,
                    current_path.clone(),
                    data.remove_trees.clone(),
                )
            }
            for value in data.values_to_remove.iter() {
                use crate::registry_utils::remove_value_in_registry;

                removed += remove_value_in_registry(&root, current_path.clone(), value.to_string());
            }
            for value in data.keys_to_remove.iter() {
                removed += remove_key_in_registry(&root, current_path.clone() + "\\" + value);
            }
        }
    }

    if removed > 0 {
        result.working = true;
        result.bytes = removed;
    }

    result
}
