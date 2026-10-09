#[cfg(windows)]
use crate::registry_utils::{
    RegistryRemoval, expand_registry_path_pattern, remove_all_in_registry,
    remove_all_in_tree_in_registry, remove_key_in_registry, remove_trees_matching_in_registry,
    remove_value_in_registry, remove_values_matching_in_registry,
};
#[cfg(windows)]
use crate::streaming::for_each_array;
#[cfg(windows)]
use crate::structures::CleanerDataRegistry;
#[cfg(windows)]
use crate::structures::RegistryIndex;
#[cfg(windows)]
use crate::structures::{CleanerResult, ClearedPath};
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

/// The hives an entry may name, as the predef handle the helpers work from.
#[cfg(windows)]
const HIVES: [(&str, winreg::HKEY); 5] = [
    ("HKEY_CURRENT_USER", HKEY_CURRENT_USER),
    ("HKEY_CURRENT_CONFIG", HKEY_CURRENT_CONFIG),
    ("HKEY_LOCAL_MACHINE", HKEY_LOCAL_MACHINE),
    ("HKEY_CLASSES_ROOT", HKEY_CLASSES_ROOT),
    ("HKEY_USERS", HKEY_USERS),
];

/// Splits an entry's path into its hive and the path relative to it.
///
/// `None` for anything that names no known root. Both halves are needed: the
/// handle is what the removals are performed through, and the hive's name is
/// what has to be printed in front of every removed key — a registry path
/// without it is not something a reader can paste into regedit.
#[cfg(windows)]
fn split_hive(path: &crate::structures::SharedPath) -> Option<(&'static str, RegKey, String)> {
    let full = path.as_string();
    for (name, handle) in HIVES {
        // Only the hive plus a separator counts: a key called
        // `HKEY_CURRENT_USER_BACKUP` must not match `HKEY_CURRENT_USER`.
        let Some(rest) = full.strip_prefix(name) else {
            continue;
        };
        if !rest.starts_with('\\') {
            continue;
        }
        let relative = rest.trim_start_matches('\\');
        // A bare hive with nothing under it: there is no key to clean, and
        // treating the empty remainder as "the root" would aim a removal at
        // every value in the hive.
        if relative.is_empty() {
            return None;
        }
        return Some((name, RegKey::predef(handle), relative.to_string()));
    }
    None
}

#[cfg(windows)]
pub fn clear_registry(data: &CleanerDataRegistry) -> CleanerResult {
    // INFO: Creating output struct
    let mut result = CleanerResult {
        files: 0,
        folders: 0,
        bytes: 0,
        working: false,
        path: data.path.clone(),
        paths: Vec::new(),
        paths_omitted: 0,
        program: data.program.clone(),
        category: data.category.clone(),
        sub_category: data.sub_category.clone(),
        // A registry key is never "locked" the way a file is, and a scan does
        // not walk the registry ahead of the run, so there is nothing to count
        // here. The bytes are zero either way, so the claim a scan makes about
        // registry entries is unchanged: it reports the keys it would remove.
        locked_files: 0,
        locked_bytes: 0,
        locked: Vec::new(),
    };

    // INFO: Every item this entry removed, named. Collected rather than summed as
    // it happens, because the results page has to show which key or value freed
    // what — and a single total cannot answer that.
    let mut removals: Vec<RegistryRemoval> = Vec::new();

    // INFO: Main logic
    if let Some((hive, root, path)) = split_hive(&data.path) {
        // INFO: Expand glob pattern in main path ("*" and "?" per segment)
        let paths: Vec<String> = if path.contains('*') || path.contains('?') {
            expand_registry_path_pattern(&root, &path)
        } else {
            vec![path.clone()]
        };

        for current_path in paths {
            if data.remove_all_in_tree {
                removals.extend(remove_all_in_tree_in_registry(&root, &current_path));
            }
            if data.remove_all_in_registry {
                removals.extend(remove_all_in_registry(&root, &current_path));
            }
            // INFO: remove_values is the glob for value names matched at
            // the end of resolved paths, "true" removes all values
            if data.remove_values == "true" {
                removals.extend(remove_all_in_registry(&root, &current_path));
            } else if !data.remove_values.is_empty() {
                removals.extend(remove_values_matching_in_registry(
                    &root,
                    &current_path,
                    &data.remove_values,
                ));
            }
            // INFO: remove_trees is the glob for subkey trees matched at
            // the end of resolved paths, "true" removes resolved keys
            if data.remove_trees == "true" {
                removals.extend(remove_key_in_registry(&root, &current_path));
            } else if !data.remove_trees.is_empty() {
                removals.extend(remove_trees_matching_in_registry(
                    &root,
                    &current_path,
                    &data.remove_trees,
                ));
            }
            for value in data.values_to_remove.iter() {
                removals.extend(remove_value_in_registry(&root, &current_path, value));
            }
            for value in data.keys_to_remove.iter() {
                removals.extend(remove_key_in_registry(
                    &root,
                    &format!("{}\\{}", current_path, value),
                ));
            }
        }

        // INFO: One report entry per removed item, each with the hive in front of
        // it. A registry cleaner removes values and subkeys rather than files, so
        // the counts stay zero — what the page can honestly report is the key.
        result.paths = removals
            .into_iter()
            .map(|removal| ClearedPath {
                path: crate::structures::SharedPath::new(&format!("{}\\{}", hive, removal.path)),
                removed_bytes: removal.bytes,
                removed_files: 0,
                removed_directories: 0,
            })
            .collect();
    }

    // INFO: The total is the sum of what was listed, never a separate count: two
    // totals that can disagree is the bug this whole path was fixed to avoid.
    result.bytes = result.paths.iter().map(|entry| entry.removed_bytes).sum();
    result.working = !result.paths.is_empty();

    result
}
