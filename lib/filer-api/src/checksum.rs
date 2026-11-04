// This is free and unencumbered software released into the public domain.

//! Checksums that drivers share (D53), in pure Rust and without
//! allocation: `compute-crc`'s, re-exported (D64).
//!
//! [`Crc32c`] is CRC-32C (Castagnoli), which XFS and ext4 stamp on their
//! metadata and iSCSI on its frames. [`Crc32`] is CRC-32 (ISO-HDLC), which
//! ZIP stamps on members and GPT on its partition tables. [`Crc16Modbus`]
//! is CRC-16/MODBUS, which ext4 stamps on group descriptors without
//! `metadata_csum`.

pub use compute_crc::{Crc16Modbus, Crc32, Crc32c, crc16_modbus, crc32, crc32c};
