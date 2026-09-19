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

/// A seeding step failed at `path`. `copy_source_path` is set only for a
/// copy failure, since that is the one step with two paths, either of which
/// can be the one the I/O error actually names (the shipped file could be
/// unreadable, or the destination could be unwritable). `source` is the
/// underlying I/O error.
#[derive(Debug)]
pub struct SeedError {
    pub path: PathBuf,
    pub copy_source_path: Option<PathBuf>,
    pub source: io::Error,
}

impl std::fmt::Display for SeedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.copy_source_path {
            Some(copy_source_path) => write!(
                f,
                "seeding failed copying {} to {}: {}",
                copy_source_path.display(),
                self.path.display(),
                self.source
            ),
            None => write!(
                f,
                "seeding failed at {}: {}",
                self.path.display(),
                self.source
            ),
        }
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
        copy_source_path: None,
        source,
    })?;

    copy_dir_excluding_working(shipped_data_dir, &dest_data_dir, true)?;

    let working_dir = dest_data_dir.join(WORKING_DIR_NAME);
    fs::create_dir_all(&working_dir).map_err(|source| SeedError {
        path: working_dir,
        copy_source_path: None,
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
        copy_source_path: None,
        source,
    })?;

    for entry in entries {
        let entry = entry.map_err(|source| SeedError {
            path: src.to_path_buf(),
            copy_source_path: None,
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
            copy_source_path: None,
            source,
        })?;

        if file_type.is_dir() {
            fs::create_dir_all(&dest_path).map_err(|source| SeedError {
                path: dest_path.clone(),
                copy_source_path: None,
                source,
            })?;
            copy_dir_excluding_working(&src_path, &dest_path, false)?;
        } else if file_type.is_file() {
            fs::copy(&src_path, &dest_path).map_err(|source| SeedError {
                path: dest_path.clone(),
                copy_source_path: Some(src_path.clone()),
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
    fn a_copy_failure_is_returned_and_names_both_paths() {
        // A pre-existing directory at the destination file's path is a
        // portable way to make `fs::copy` fail (it cannot overwrite a
        // directory), without relying on permission bits root ignores.
        let shipped = tempfile::tempdir().expect("create shipped dir");
        let user_root = tempfile::tempdir().expect("create user root dir");
        write_file(shipped.path(), "blocked.toml", "profile = 1\n");
        fs::create_dir_all(user_root.path().join("data/blocked.toml"))
            .expect("create directory standing in for the destination file");

        let err = seed_data(shipped.path(), user_root.path()).expect_err("seed should fail");

        let message = err.to_string();
        let expected_src = shipped.path().join("blocked.toml");
        let expected_dest = user_root.path().join("data/blocked.toml");
        assert!(
            message.contains(&expected_src.display().to_string()),
            "message {message:?} does not name the source path {}",
            expected_src.display()
        );
        assert!(
            message.contains(&expected_dest.display().to_string()),
            "message {message:?} does not name the destination path {}",
            expected_dest.display()
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
