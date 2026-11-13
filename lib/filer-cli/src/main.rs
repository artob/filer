// This is free and unencumbered software released into the public domain.

//! The `filer` command-line interface.
//!
//! This binary will expose the Filer API from the command line through
//! `filer-system`, which selects the platform's native driver. It requires
//! the `std` feature, enabled by default. It does nothing yet.

#![forbid(unsafe_code)]

// The CLI is to work through `filer-system` (D30); until it does, this
// keeps Cargo from warning that the dependency goes unused.
use filer_system as _;

fn main() {}
