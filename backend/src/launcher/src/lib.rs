//! Platform-neutral logic for the windowless launcher (issue #62). Win32
//! process supervision, health polling, the tray and the port-change dialog
//! land in later slices; this crate currently holds settings resolution and
//! data seeding only, both unit-tested on Linux.

pub mod seed;
pub mod settings;
