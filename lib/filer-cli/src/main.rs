// This is free and unencumbered software released into the public domain.

//! The `filer` command-line interface.
//!
//! This binary will expose the Filer API from the command line, using the
//! platform's backend: `filer-posix` on Unix, `filer-wasi` on WASI, and
//! `filer-windows` on Windows. It requires the `std` feature, enabled by
//! default. It does nothing yet.

#![forbid(unsafe_code)]

fn main() {}
