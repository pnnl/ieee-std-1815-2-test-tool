//! Type-safe DNP3 point index enums and UID utilities generated from the PICS profile.
//!
//! The `ai_uid`, `bi_uid`, `ao_uid`, and `bo_uid` modules contain `#[repr(u16)]` enums
//! whose variants are named after IEC 61850 UIDs and whose discriminants are the
//! corresponding DNP3 point indices. These are generated at build time by `build.rs`.

pub mod ai_uid {
    include!(concat!(env!("OUT_DIR"), "/aiuid.rs"));
}

pub mod bi_uid {
    include!(concat!(env!("OUT_DIR"), "/biuid.rs"));
}

pub mod ao_uid {
    include!(concat!(env!("OUT_DIR"), "/aouid.rs"));
}

pub mod bo_uid {
    include!(concat!(env!("OUT_DIR"), "/bouid.rs"));
}
