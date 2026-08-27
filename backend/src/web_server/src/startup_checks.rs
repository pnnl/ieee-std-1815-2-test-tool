//! Startup security checks.
//!
//! These run once during boot, before the HTTP server is constructed. Any
//! failure here aborts startup with a clear error rather than silently
//! exposing unsafe behavior at request time.

use std::io;
use std::path::Path;

/// Walk `dir` recursively and reject any symbolic link found beneath it.
///
/// Poem's `StaticFilesEndpoint` follows symlinks when resolving file paths:
/// it strips URL traversal (`..`) but never canonicalizes the on-disk path
/// it ends up opening. A symlink dropped inside a served directory therefore
/// escapes the static-files sandbox to anywhere the process can read. We
/// scan eagerly at startup so the failure mode is "service refuses to come
/// up" instead of "service silently serves /etc/passwd".
///
/// `label` is surfaced in the error so operators can tell which configured
/// directory tripped the check (e.g. `FRONTEND_DIR` vs `DATA_DIR`).
///
/// If `dir` itself does not exist this returns `Ok(())`: callers may invoke
/// `web_server` before `make build` has produced `frontend/dist`, and the
/// static-files endpoint already returns clean 404s in that case.
pub fn assert_no_symlinks(label: &str, dir: &Path) -> io::Result<()> {
    // Inspect the root with `symlink_metadata` (does NOT follow symlinks) so a
    // symlinked root directory is itself rejected. `Path::exists()` follows
    // links and would silently let the scan walk the target -- defeating the
    // sandbox if FRONTEND_DIR or DATA_DIR is configured as a symlink.
    let meta = match std::fs::symlink_metadata(dir) {
        Ok(m) => m,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            // Missing dir is fine: callers may invoke web_server before
            // `make build` has produced frontend/dist, and the static-files
            // endpoint already returns clean 404s in that case.
            return Ok(());
        }
        Err(e) => return Err(e),
    };
    if meta.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!(
                "{label} root is a symlink and is rejected for security: {} \
                 (point {label} at a real directory instead of a symlink)",
                dir.display()
            ),
        ));
    }
    scan(label, dir)
}

fn scan(label: &str, dir: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        // symlink_metadata does NOT follow symlinks; file_type().is_symlink()
        // therefore reports the link itself rather than its target.
        let meta = std::fs::symlink_metadata(&path)?;
        let file_type = meta.file_type();
        if file_type.is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                format!(
                    "Symlinks under {label} are rejected for security: {} \
                     (remove the symlink or set {label} to a clean directory)",
                    path.display()
                ),
            ));
        }
        if file_type.is_dir() {
            scan(label, &path)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    #[test]
    fn test_scan_rejects_symlink_in_dir() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "real file").expect("write target");
        let link = dir.path().join("link.txt");
        symlink(&target, &link).expect("create symlink");

        let err =
            assert_no_symlinks("TEST_DIR", dir.path()).expect_err("expected symlink rejection");
        let msg = err.to_string();
        assert!(
            msg.contains("link.txt"),
            "error should name the symlink path, got: {msg}"
        );
        assert!(
            msg.contains("TEST_DIR"),
            "error should name the label, got: {msg}"
        );
    }

    #[test]
    fn test_scan_accepts_clean_dir() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(dir.path().join("a.txt"), "a").expect("write a");
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).expect("mkdir sub");
        std::fs::write(sub.join("b.txt"), "b").expect("write b");

        assert_no_symlinks("TEST_DIR", dir.path()).expect("clean dir should pass");
    }

    #[test]
    fn test_scan_handles_missing_dir() {
        // Build a path that's guaranteed not to exist by joining a child under
        // a freshly-created tempdir; avoids a hardcoded /tmp path that could
        // collide on shared hosts or leak between test runs.
        let parent = tempfile::tempdir().expect("create temp dir");
        let missing = parent.path().join("definitely-missing");
        assert!(!missing.exists(), "precondition: path should not exist");
        assert_no_symlinks("TEST_DIR", &missing).expect("missing dir should pass");
    }

    #[test]
    fn test_scan_rejects_symlinked_root_dir() {
        // If the configured directory is itself a symlink, the old `dir.exists()`
        // check happily followed it and walked the target without flagging the
        // root as a link. That defeats the sandboxing intent: an operator who
        // points FRONTEND_DIR at a symlink could escape the static-files sandbox.
        let parent = tempfile::tempdir().expect("create parent temp dir");
        let real = tempfile::tempdir().expect("create real target dir");
        let link_dir = parent.path().join("link");
        symlink(real.path(), &link_dir).expect("create root symlink");

        let err = assert_no_symlinks("TEST_DIR", &link_dir)
            .expect_err("symlinked root should be rejected");
        let msg = err.to_string();
        assert!(
            msg.contains(&link_dir.display().to_string()),
            "error should name the symlinked root path, got: {msg}"
        );
        assert!(
            msg.contains("TEST_DIR"),
            "error should name the label, got: {msg}"
        );
    }

    #[test]
    fn test_scan_descends_into_subdirs() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let sub = dir.path().join("nested").join("deeper");
        std::fs::create_dir_all(&sub).expect("mkdir nested/deeper");
        let target = dir.path().join("target.txt");
        std::fs::write(&target, "real file").expect("write target");
        let link = sub.join("escape.txt");
        symlink(&target, &link).expect("create nested symlink");

        let err = assert_no_symlinks("TEST_DIR", dir.path())
            .expect_err("expected symlink rejection in subdir");
        assert!(
            err.to_string().contains("escape.txt"),
            "error should name the nested symlink, got: {err}"
        );
    }
}
