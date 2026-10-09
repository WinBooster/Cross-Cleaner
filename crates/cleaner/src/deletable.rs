//! Can this file actually be removed?
//!
//! A dry run that reports a file as deletable when a running application is
//! holding it open is worse than no dry run: the user is told a size they will
//! not get. This module answers the question the size cannot — does a removal
//! of this path have a chance of succeeding right now.
//!
//! What "chance" means differs by platform, and each platform is asked its own
//! question rather than approximated with the other's:
//!
//! * **Windows.** A file is deletable when it can be opened with `DELETE`
//!   access *and* `FILE_SHARE_DELETE` on every existing handle. A browser
//!   holding its cache database open without share-delete is the case this
//!   exists for. The probe opens the handle and closes it again — it never
//!   deletes, and it never blocks: `FILE_FLAG_BACKUP_SEMANTICS` lets it ask
//!   about directories too.
//! * **Everything else.** Unlinking a file needs write permission on its
//!   *parent directory*, not on the file itself. A read-only file inside a
//!   writable directory is removable; a writable file inside a read-only one is
//!   not. The probe asks exactly that, since asking about the file would give
//!   the wrong answer in both directions.
//!
//! The result is advisory, and the module says so: a file can be locked a
//! millisecond after this returns. It is reported as "what the scan found",
//! never as a guarantee.

use std::path::Path;

/// Whether a path can be removed at the moment it was asked about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Deletable {
    /// Removal should succeed.
    Yes,
    /// Something is holding it: another process has it open on Windows, or the
    /// parent directory refuses new entries on Unix.
    Locked,
    /// The file's own permissions refuse the removal. On Unix this is rare for
    /// deletion and usually means a directory is not writable at all.
    Denied,
    /// The probe could not tell — a path that vanished mid-scan, or an error
    /// that is neither "locked" nor "denied".
    Unknown,
}

impl Deletable {
    /// True when the probe found no reason the removal would fail.
    pub fn is_yes(self) -> bool {
        self == Deletable::Yes
    }

    /// A short phrase for a report, saying what to do about it.
    pub fn reason(self) -> &'static str {
        match self {
            Deletable::Yes => "removable",
            // INFO: the two failure modes need different advice. A locked file
            // is fixed by closing the application; a denied one is fixed by
            // running elevated. Printing one message for both sends the user
            // after the wrong one.
            Deletable::Locked => "in use by another program",
            Deletable::Denied => "permission denied",
            Deletable::Unknown => "could not be checked",
        }
    }
}

/// Asks whether `path` can be removed right now. Never removes anything.
///
/// A directory is probed too, and the probe answers a narrower question for one.
/// Whether it is *empty* is the walk's problem, and on Windows a handle held on
/// a directory does not block a `DELETE` open the way it blocks one on a file —
/// see `a_directory_open_does_not_enforce_share_mode`. What the probe does catch
/// is the ACL refusing the delete, which is the failure that actually kept a
/// scanned directory from being removed. Counting such a directory as freeable
/// promised bytes the run would not deliver.
pub fn is_deletable(path: &Path) -> Deletable {
    let Ok(meta) = path.symlink_metadata() else {
        // It is gone already, or was never there. Reporting "removable" would
        // be a claim about a file the run will not find either — so this is the
        // one answer that has to say it does not know.
        return Deletable::Unknown;
    };
    if meta.is_file() || meta.is_dir() {
        return probe(path);
    }
    // Sockets, devices and the like: nothing to ask about, and nothing the walk
    // would remove as a plain entry either.
    Deletable::Yes
}

#[cfg(windows)]
fn probe(path: &Path) -> Deletable {
    use std::os::windows::ffi::OsStrExt;
    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::Storage::FileSystem::{
        CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_DELETE, FILE_SHARE_READ,
        FILE_SHARE_WRITE, OPEN_EXISTING,
    };
    use windows::core::{HRESULT, PCWSTR};

    /// `DELETE` from `winnt.h`, spelled out so the intent is visible at the
    /// call site rather than hidden behind an import. `windows` exposes it as
    /// `DELETE` only under a feature this crate does not otherwise need.
    const DELETE_ACCESS: u32 = 0x0001_0000;
    /// `ERROR_SHARING_VIOLATION`: another process holds a handle without
    /// `FILE_SHARE_DELETE`. This is the browser-with-its-cache-open case.
    const ERROR_SHARING_VIOLATION: u32 = 32;
    /// `ERROR_ACCESS_DENIED`: the ACL refuses this user the delete.
    const ERROR_ACCESS_DENIED: u32 = 5;

    let wide: Vec<u16> = path
        .as_os_str()
        .encode_wide()
        .chain(std::iter::once(0))
        .collect();

    // Share *all* modes on the probe handle: it is asking "could a deleter get
    // in", and refusing to share would make the probe itself the obstacle it is
    // trying to measure.
    let share = FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE;

    // SAFETY: `wide` is NUL-terminated and outlives the call, and every other
    // value is a plain constant. `CreateFileW` here is only ever *opening* the
    // file — no `FILE_FLAG_DELETE_ON_CLOSE`, no delete disposition — and the
    // handle is closed before returning on the success path.
    let handle = unsafe {
        CreateFileW(
            PCWSTR(wide.as_ptr()),
            DELETE_ACCESS,
            share,
            None,
            OPEN_EXISTING,
            FILE_FLAG_BACKUP_SEMANTICS,
            None,
        )
    };

    let handle = match handle {
        Ok(handle) => handle,
        Err(error) => {
            // A `GetLastError` code arrives wrapped in an HRESULT, so the two
            // failures have to be compared as wrapped codes. Comparing
            // `error.code().0` against the bare Win32 value would match nothing
            // and every failure would come back "could not be checked".
            return if error.code() == HRESULT::from_win32(ERROR_SHARING_VIOLATION) {
                Deletable::Locked
            } else if error.code() == HRESULT::from_win32(ERROR_ACCESS_DENIED) {
                Deletable::Denied
            } else {
                Deletable::Unknown
            };
        }
    };

    // Opened successfully means the delete access was granted, which is the
    // answer. Closing it right away is what makes this a probe rather than a
    // deletion.
    // SAFETY: `handle` came back `Ok` from `CreateFileW` and is closed once.
    unsafe {
        let _ = CloseHandle(handle);
    }
    Deletable::Yes
}

#[cfg(not(windows))]
fn probe(path: &Path) -> Deletable {
    // Unlinking needs write and search permission on the parent directory, not
    // on the file. This is the case that makes the difference: a read-only file
    // in a writable directory is removable, and a writable file in a read-only
    // directory is not — so probing the file itself would answer both backwards.
    let Some(parent) = path.parent() else {
        return Deletable::Unknown;
    };
    let Ok(meta) = parent.metadata() else {
        return Deletable::Unknown;
    };
    if meta.permissions().readonly() {
        Deletable::Denied
    } else {
        Deletable::Yes
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn a_plain_file_is_deletable() {
        let dir = TempDir::new().unwrap();
        let file = dir.path().join("a.txt");
        fs::write(&file, b"x").unwrap();
        assert!(is_deletable(&file).is_yes());
    }

    #[test]
    fn a_file_that_does_not_exist_is_not_reported_as_removable() {
        let dir = TempDir::new().unwrap();
        let gone = dir.path().join("gone.txt");
        assert_eq!(is_deletable(&gone), Deletable::Unknown);
    }

    #[test]
    fn a_directory_is_probed_like_a_file() {
        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        // A plain directory is removable. Whether it is empty is the walk's
        // question; the lock and the ACL are this probe's, and both apply.
        assert_eq!(is_deletable(&sub), Deletable::Yes);
    }

    /// Windows does not enforce share-mode on a directory open the way it does on
    /// a file: holding a directory with `FILE_SHARE_MODE(0)` still lets a
    /// `DELETE` open succeed, while the same handle on a file does not (see
    /// `an_open_handle_without_share_delete_is_reported_as_locked` above).
    ///
    /// So for a directory this probe answers access and not locking, and the
    /// `Locked` branch is unreachable there. Pinned because the asymmetry is
    /// surprising and the obvious thing is to "fix" the probe by assuming the
    /// file behaviour and shipping a claim it cannot back up.
    #[cfg(windows)]
    #[test]
    fn a_directory_open_does_not_enforce_share_mode() {
        use std::os::windows::ffi::OsStrExt;
        use windows::Win32::Storage::FileSystem::{
            CreateFileW, FILE_FLAG_BACKUP_SEMANTICS, FILE_SHARE_MODE, OPEN_EXISTING,
        };
        use windows::core::PCWSTR;

        let dir = TempDir::new().unwrap();
        let sub = dir.path().join("held");
        fs::create_dir(&sub).unwrap();

        let wide: Vec<u16> = sub
            .as_os_str()
            .encode_wide()
            .chain(std::iter::once(0))
            .collect();
        // SAFETY: `wide` is NUL-terminated and outlives the call.
        let held = unsafe {
            CreateFileW(
                PCWSTR(wide.as_ptr()),
                0,
                FILE_SHARE_MODE(0),
                None,
                OPEN_EXISTING,
                FILE_FLAG_BACKUP_SEMANTICS,
                None,
            )
        }
        .expect("the directory opens");

        // Documented behaviour, not an accident of the test: no `Locked`.
        assert!(is_deletable(&sub).is_yes());

        // SAFETY: the handle came back from `CreateFileW` and is closed once.
        unsafe {
            let _ = windows::Win32::Foundation::CloseHandle(held);
        }
    }

    #[test]
    fn every_outcome_has_advice() {
        for state in [
            Deletable::Yes,
            Deletable::Locked,
            Deletable::Denied,
            Deletable::Unknown,
        ] {
            assert!(!state.reason().is_empty(), "{state:?} has no reason");
        }
        // The two failures must not read the same, or the user is sent after
        // the wrong fix.
        assert_ne!(Deletable::Locked.reason(), Deletable::Denied.reason());
    }

    /// On Windows a handle opened without `FILE_SHARE_DELETE` is exactly what a
    /// running application looks like, and it is the case this probe exists for.
    #[cfg(windows)]
    #[test]
    fn an_open_handle_without_share_delete_is_reported_as_locked() {
        use std::os::windows::fs::OpenOptionsExt;

        let dir = TempDir::new().unwrap();
        let file = dir.path().join("held.txt");
        fs::write(&file, b"x").unwrap();

        // `File::open` shares delete by default, so it creates no conflict.
        // `share_mode` is what does: no sharing at all means no deleter can
        // open the file while this handle lives.
        let held = fs::OpenOptions::new()
            .read(true)
            .share_mode(0)
            .open(&file)
            .expect("the file opens");
        assert_eq!(is_deletable(&file), Deletable::Locked);
        drop(held);

        // Released, the same file is removable again — which is what makes the
        // answer a statement about *now* rather than about the file.
        assert!(is_deletable(&file).is_yes());
    }
}
