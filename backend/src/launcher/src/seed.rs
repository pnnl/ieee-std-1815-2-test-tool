//! Copies the shipped data seed into the user's data root, mirroring
//! `launch.cmd`'s robocopy step: overwrite existing seed files, never touch
//! an existing `working` subdirectory, and create `working` if it is
//! missing. Both directories are parameters so this is testable with plain
//! temp directories.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Name of the subdirectory under the destination's `data` directory that
/// seeding must never read, write or delete, except to create it when it is
/// absent.
const WORKING_DIR_NAME: &str = "working";

/// A seeding step failed at `path`. `source` is the underlying I/O error.
#[derive(Debug)]
pub struct SeedError {
    pub path: PathBuf,
    pub source: io::Error,
}

impl std::fmt::Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "seeding failed at {}: {}",
            self.path.display(),
            self.source
        )
    }
}

impl std::error::Error for SeedError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.source)
    }
}

/// Seed `user_data_root/data` from `shipped_data_dir`.
///
/// Every entry in `shipped_data_dir` is copied into the destination
/// recursively, overwriting existing files. A top-level `working` entry in
/// the source, if one exists, is never copied, so a stray `working` in the
/// shipped seed can never overwrite the destination's own. The destination's
/// `working` directory is created if it is missing and is otherwise never
/// read, written or deleted by this function.
pub fn seed_data(shipped_data_dir: &Path, user_data_root: &Path) -> Result<(), SeedError> {
    let dest_data_dir = user_data_root.join("data");
    fs::create_dir_all(&dest_data_dir).map_err(|source| SeedError {
        path: dest_data_dir.clone(),
        source,
    })?;

    copy_dir_excluding_working(shipped_data_dir, &dest_data_dir, true)?;

    let working_dir = dest_data_dir.join(WORKING_DIR_NAME);
    fs::create_dir_all(&working_dir).map_err(|source| SeedError {
        path: working_dir,
        source,
    })?;

    Ok(())
}

/// Recursively copy `src` into `dest`, overwriting existing files. When
/// `top_level` is true, an entry directly under `src` named `working` is
/// skipped rather than copied.
fn copy_dir_excluding_working(src: &Path, dest: &Path, top_level: bool) -> Result<(), SeedError> {
    let entries = fs::read_dir(src).map_err(|source| SeedError {
        path: src.to_path_buf(),
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| SeedError {
            path: src.to_path_buf(),
            source,
        })?;
        let file_name = entry.file_name();

        if top_level && file_name == WORKING_DIR_NAME {
            continue;
        }

        let src_path = entry.path();
        let dest_path = dest.join(&file_name);
        let file_type = entry.file_type().map_err(|source| SeedError {
            path: src_path.clone(),
            source,
        })?;

        if file_type.is_dir() {
            fs::create_dir_all(&dest_path).map_err(|source| SeedError {
                path: dest_path.clone(),
                source,
            })?;
            copy_dir_excluding_working(&src_path, &dest_path, false)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dest_path).map_err(|source| SeedError {
                path: dest_path.clone(),
                source,
            })?;
        }
        // A symlink or other special entry is neither expected in the
        // shipped seed nor handled here; it is skipped rather than copied or
        // erroring, the same as a plain file-by-file copy would do.
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn write_file(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent dir");
        }
        fs::write(&path, contents).expect("write fixture file");
    }

    fn read_file(dir: &Path, rel: &str) -> String {
        fs::read_to_string(dir.join(rel)).expect("read fixture file")
    }

    #[test]
    fn copies_shipped_files_into_the_user_data_root() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/default.toml", "profile = 1\n");
        write_file(shipped.path(), "README.md", "seed readme\n");

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert_eq!(
            read_file(user_root.path(), "data/profiles/default.toml"),
            "profile = 1\n"
        );
        assert_eq!(
            read_file(user_root.path(), "data/README.md"),
            "seed readme\n"
        );
    }

    #[test]
    fn overwrites_existing_seed_files() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/default.toml", "profile = 2\n");
        write_file(
            user_root.path(),
            "data/profiles/default.toml",
            "profile = 1 (stale)\n",
        );

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert_eq!(
            read_file(user_root.path(), "data/profiles/default.toml"),
            "profile = 2\n"
        );
    }

    #[test]
    fn creates_working_when_missing() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/default.toml", "profile = 1\n");

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert!(user_root.path().join("data/working").is_dir());
    }

    #[test]
    fn never_touches_an_existing_working_directory() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/default.toml", "profile = 1\n");
        write_file(
            user_root.path(),
            "data/working/job-42.json",
            "in progress\n",
        );

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert_eq!(
            read_file(user_root.path(), "data/working/job-42.json"),
            "in progress\n"
        );
    }

    #[test]
    fn a_working_entry_in_the_shipped_seed_is_never_copied() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/default.toml", "profile = 1\n");
        write_file(
            shipped.path(),
            "working/should-not-land.txt",
            "stray shipped working\n",
        );
        write_file(
            user_root.path(),
            "data/working/job-42.json",
            "in progress\n",
        );

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert!(
            !user_root
                .path()
                .join("data/working/should-not-land.txt")
                .exists()
        );
        assert_eq!(
            read_file(user_root.path(), "data/working/job-42.json"),
            "in progress\n"
        );
    }

    #[test]
    fn a_user_file_the_seed_never_shipped_survives_seeding() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/default.toml", "profile = 1\n");
        write_file(
            user_root.path(),
            "data/profiles/user-notes.txt",
            "kept beside the seed\n",
        );
        write_file(
            user_root.path(),
            "data/working/job-42.json",
            "in progress\n",
        );

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert_eq!(
            read_file(user_root.path(), "data/profiles/user-notes.txt"),
            "kept beside the seed\n"
        );
        assert_eq!(
            read_file(user_root.path(), "data/working/job-42.json"),
            "in progress\n"
        );
    }

    #[test]
    fn nested_directories_are_copied_recursively() {
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "profiles/nested/deep.toml", "deep = true\n");

        seed_data(shipped.path(), user_root.path()).expect("seed");

        assert_eq!(
            read_file(user_root.path(), "data/profiles/nested/deep.toml"),
            "deep = true\n"
        );
    }
}
