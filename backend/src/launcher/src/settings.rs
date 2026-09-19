//! Effective port resolution: the `PORT` environment variable, then the
//! per-user settings file, then the installer-written default file, then
//! 8000. Each layer is tried in that order; a layer that exists but is
//! malformed or holds a port outside 1024-65535 is an error naming that
//! layer, never a silent fall-through to the layer below it.

use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// Valid launcher ports sit in this range: below 1024 needs elevated
/// privileges on most platforms, and the built-in default (8000) sits well
/// inside it.
pub const MIN_PORT: u16 = 1024;
pub const MAX_PORT: u16 = 65535;

/// Used when no other layer sets a port.
pub const DEFAULT_PORT: u16 = 8000;

/// Which layer produced the effective port, carried into error and (later)
/// tray-menu messages so a reader can see where a value came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortSource {
    Env,
    UserFile(PathBuf),
    InstallDefaultFile(PathBuf),
    BuiltinDefault,
}

impl fmt::Display for PortSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PortSource::Env => write!(f, "the PORT environment variable"),
            PortSource::UserFile(path) => {
                write!(f, "the per-user settings file at {}", path.display())
            }
            PortSource::InstallDefaultFile(path) => {
                write!(f, "the install default file at {}", path.display())
            }
            PortSource::BuiltinDefault => write!(f, "the built-in default"),
        }
    }
}

/// The resolved effective port and which layer produced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPort {
    pub port: u16,
    pub source: PortSource,
}

/// A settings layer that exists could not be used as-is. Resolution stops
/// here rather than trying the next layer down, so a value the user actually
/// set is never silently overridden by a lower-priority default.
#[derive(Debug, PartialEq, Eq)]
pub enum SettingsError {
    /// The file at `path` exists but could not be opened or read (permission
    /// denied, the path is a directory, and similar I/O failures other than
    /// the file not existing).
    UnreadableFile { path: PathBuf, detail: String },
    /// The file at `path` was read but could not be parsed as TOML.
    MalformedFile { path: PathBuf, detail: String },
    /// The `port` key at `source` was not an integer in 1024-65535.
    InvalidPort { source: PortSource, value: String },
}

impl fmt::Display for SettingsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SettingsError::UnreadableFile { path, detail } => {
                write!(
                    f,
                    "settings file {} could not be read: {}",
                    path.display(),
                    detail
                )
            }
            SettingsError::MalformedFile { path, detail } => {
                write!(
                    f,
                    "settings file {} is malformed: {}",
                    path.display(),
                    detail
                )
            }
            SettingsError::InvalidPort { source, value } => write!(
                f,
                "port {value:?} from {source} must be an integer between {MIN_PORT} and {MAX_PORT}"
            ),
        }
    }
}

impl std::error::Error for SettingsError {}

/// Resolve the effective port.
///
/// `env_port` is the raw `PORT` environment variable value, read by the
/// caller so this function has no dependency on process-global state and is
/// testable with plain arguments. `user_file` and `install_default_file` are
/// read only if they exist; a missing file is not an error, it just means
/// that layer defers to the layer below it. A file that exists is read even
/// when a higher-priority layer never needs to look at it, except that this
/// function stops at the first layer that produces a value (present in the
/// environment, or an existing file), so a lower layer is never read once a
/// higher one has answered.
pub fn resolve_port(
    env_port: Option<&str>,
    user_file: &Path,
    install_default_file: &Path,
) -> Result<ResolvedPort, SettingsError> {
    if let Some(raw) = env_port {
        let port = parse_port(raw).ok_or_else(|| SettingsError::InvalidPort {
            source: PortSource::Env,
            value: raw.to_string(),
        })?;
        return Ok(ResolvedPort {
            port,
            source: PortSource::Env,
        });
    }

    if let Some(port) = read_port_file(user_file, PortSource::UserFile(user_file.to_path_buf()))? {
        return Ok(ResolvedPort {
            port,
            source: PortSource::UserFile(user_file.to_path_buf()),
        });
    }

    if let Some(port) = read_port_file(
        install_default_file,
        PortSource::InstallDefaultFile(install_default_file.to_path_buf()),
    )? {
        return Ok(ResolvedPort {
            port,
            source: PortSource::InstallDefaultFile(install_default_file.to_path_buf()),
        });
    }

    Ok(ResolvedPort {
        port: DEFAULT_PORT,
        source: PortSource::BuiltinDefault,
    })
}

/// Read the `port` key from a TOML settings file at `path`. `Ok(None)` means
/// the file does not exist, or exists with no `port` key, so the caller
/// falls through to the next layer. Any other read or parse failure, or a
/// `port` value outside 1024-65535, is an error naming `source`.
fn read_port_file(path: &Path, source: PortSource) -> Result<Option<u16>, SettingsError> {
    let text = match fs::read_to_string(path) {
        Ok(text) => text,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(SettingsError::UnreadableFile {
                path: path.to_path_buf(),
                detail: err.to_string(),
            });
        }
    };

    let table: toml::Value = toml::from_str(&text).map_err(|err| SettingsError::MalformedFile {
        path: path.to_path_buf(),
        detail: err.to_string(),
    })?;

    let Some(value) = table.get("port") else {
        return Ok(None);
    };

    let raw = match value {
        toml::Value::Integer(n) => n.to_string(),
        toml::Value::String(s) => s.clone(),
        other => other.to_string(),
    };

    match parse_port(&raw) {
        Some(port) => Ok(Some(port)),
        None => Err(SettingsError::InvalidPort { source, value: raw }),
    }
}

/// Parse and range-check a port string. Shared by the env layer and the file
/// layers so both apply the same 1024-65535 bound.
fn parse_port(raw: &str) -> Option<u16> {
    let n: i64 = raw.trim().parse().ok()?;
    if n < MIN_PORT as i64 || n > MAX_PORT as i64 {
        return None;
    }
    Some(n as u16)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn missing_path(dir: &tempfile::TempDir, name: &str) -> PathBuf {
        dir.path().join(name)
    }

    fn write_file(dir: &tempfile::TempDir, name: &str, contents: &str) -> PathBuf {
        let path = dir.path().join(name);
        fs::write(&path, contents).expect("write fixture file");
        path
    }

    #[test]
    fn env_layer_wins_when_present() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = write_file(&dir, "user.toml", "port = 9001\n");
        let install_file = write_file(&dir, "install.toml", "port = 9002\n");

        let resolved = resolve_port(Some("7000"), &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 7000);
        assert_eq!(resolved.source, PortSource::Env);
    }

    #[test]
    fn env_layer_wins_over_a_malformed_user_file() {
        // The design plan (section 4.4) states the precedence but is silent
        // on whether a higher layer's presence means a lower, malformed file
        // is read at all. The safe reading: precedence is "first match wins"
        // and a lower layer is never even opened once a higher one answers,
        // so a bad file the user never intended to use (or has not gotten to
        // fixing yet) cannot break a launch that does not depend on it.
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = write_file(&dir, "user.toml", "not valid toml {{{");
        let install_file = missing_path(&dir, "install.toml");

        let resolved = resolve_port(Some("7000"), &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 7000);
        assert_eq!(resolved.source, PortSource::Env);
    }

    #[test]
    fn user_file_wins_over_install_default() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = write_file(&dir, "user.toml", "port = 9001\n");
        let install_file = write_file(&dir, "install.toml", "port = 9002\n");

        let resolved = resolve_port(None, &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 9001);
        assert_eq!(resolved.source, PortSource::UserFile(user_file));
    }

    #[test]
    fn install_default_wins_when_user_file_absent() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = write_file(&dir, "install.toml", "port = 9002\n");

        let resolved = resolve_port(None, &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 9002);
        assert_eq!(
            resolved.source,
            PortSource::InstallDefaultFile(install_file)
        );
    }

    #[test]
    fn builtin_default_when_nothing_else_set() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = missing_path(&dir, "install.toml");

        let resolved = resolve_port(None, &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, DEFAULT_PORT);
        assert_eq!(resolved.source, PortSource::BuiltinDefault);
    }

    #[test]
    fn user_file_without_port_key_falls_through() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = write_file(&dir, "user.toml", "other_key = 1\n");
        let install_file = write_file(&dir, "install.toml", "port = 9002\n");

        let resolved = resolve_port(None, &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 9002);
        assert_eq!(
            resolved.source,
            PortSource::InstallDefaultFile(install_file)
        );
    }

    #[test]
    fn port_below_min_is_rejected() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = missing_path(&dir, "install.toml");

        let err = resolve_port(Some("1023"), &user_file, &install_file).unwrap_err();

        assert_eq!(
            err,
            SettingsError::InvalidPort {
                source: PortSource::Env,
                value: "1023".to_string()
            }
        );
    }

    #[test]
    fn port_at_min_is_accepted() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = missing_path(&dir, "install.toml");

        let resolved = resolve_port(Some("1024"), &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 1024);
    }

    #[test]
    fn port_at_max_is_accepted() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = missing_path(&dir, "install.toml");

        let resolved = resolve_port(Some("65535"), &user_file, &install_file).expect("resolve");

        assert_eq!(resolved.port, 65535);
    }

    #[test]
    fn port_above_max_is_rejected() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = missing_path(&dir, "install.toml");

        let err = resolve_port(Some("65536"), &user_file, &install_file).unwrap_err();

        assert_eq!(
            err,
            SettingsError::InvalidPort {
                source: PortSource::Env,
                value: "65536".to_string()
            }
        );
    }

    #[test]
    fn non_numeric_port_is_rejected() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = missing_path(&dir, "install.toml");

        let err = resolve_port(Some("abc"), &user_file, &install_file).unwrap_err();

        assert_eq!(
            err,
            SettingsError::InvalidPort {
                source: PortSource::Env,
                value: "abc".to_string()
            }
        );
    }

    #[test]
    fn install_default_port_out_of_range_is_rejected_naming_its_layer() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = write_file(&dir, "install.toml", "port = 70000\n");

        let err = resolve_port(None, &user_file, &install_file).unwrap_err();

        match err {
            SettingsError::InvalidPort {
                source: PortSource::InstallDefaultFile(path),
                value,
            } => {
                assert_eq!(path, install_file);
                assert_eq!(value, "70000");
            }
            other => panic!("expected InstallDefaultFile InvalidPort, got {other:?}"),
        }
    }

    #[test]
    fn malformed_user_file_is_reported_with_its_path() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = write_file(&dir, "user.toml", "this is not { valid toml");
        let install_file = missing_path(&dir, "install.toml");

        let err = resolve_port(None, &user_file, &install_file).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains(&user_file.display().to_string()),
            "message {message:?} does not name {}",
            user_file.display()
        );
        match err {
            SettingsError::MalformedFile { path, .. } => assert_eq!(path, user_file),
            other => panic!("expected MalformedFile, got {other:?}"),
        }
    }

    #[test]
    fn a_read_error_that_is_not_missing_stays_an_error_naming_the_path() {
        // A directory where the settings file should be is a portable way to
        // trigger a read error other than NotFound, without relying on
        // permission bits that root ignores.
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = dir.path().join("user.toml");
        fs::create_dir(&user_file).expect("create directory standing in for the file");
        let install_file = missing_path(&dir, "install.toml");

        let err = resolve_port(None, &user_file, &install_file).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains(&user_file.display().to_string()),
            "message {message:?} does not name {}",
            user_file.display()
        );
        assert!(
            !message.contains("malformed"),
            "message {message:?} calls a read failure malformed"
        );
        match err {
            SettingsError::UnreadableFile { path, .. } => assert_eq!(path, user_file),
            other => panic!("expected UnreadableFile, got {other:?}"),
        }
    }

    #[test]
    fn malformed_install_default_file_is_reported_with_its_path() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let user_file = missing_path(&dir, "user.toml");
        let install_file = write_file(&dir, "install.toml", "this is not { valid toml");

        let err = resolve_port(None, &user_file, &install_file).unwrap_err();

        let message = err.to_string();
        assert!(
            message.contains(&install_file.display().to_string()),
            "message {message:?} does not name {}",
            install_file.display()
        );
        match err {
            SettingsError::MalformedFile { path, .. } => assert_eq!(path, install_file),
            other => panic!("expected MalformedFile, got {other:?}"),
        }
    }
}
