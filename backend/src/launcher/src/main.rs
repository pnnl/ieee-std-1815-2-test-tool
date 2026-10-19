//! Stub binary. Win32 process supervision, health polling and the tray land
//! in later slices; this build target only proves the workspace still
//! produces a launcher binary on every platform, Windows included.
#![cfg_attr(windows, windows_subsystem = "windows")]

fn main() {
    #[cfg(windows)]
    {
        eprintln!("test-tool-launcher: not yet implemented (see issue #62)");
        std::process::exit(2);
    }
    #[cfg(not(windows))]
    {
        eprintln!("test-tool-launcher: the launcher is Windows only");
        std::process::exit(2);
    }
}
