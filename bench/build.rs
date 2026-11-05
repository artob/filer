// This is free and unencumbered software released into the public domain.

//! Records how the harness was built, for its results: the toolchain, the
//! Rust flags, which tell a baseline build from `target-cpu=native`, the C
//! compiler and its flags, and the version of every crate it links.

use std::{env, fs, process::Command};

fn main() {
    let first_line = |program: &str| {
        Command::new(program)
            .arg("--version")
            .output()
            .ok()
            .and_then(|output| String::from_utf8(output.stdout).ok())
            .and_then(|text| text.lines().next().map(str::to_owned))
            .unwrap_or_default()
    };
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".into());
    let rustflags = env::var("CARGO_ENCODED_RUSTFLAGS").unwrap_or_default();
    let cc = env::var("CC").unwrap_or_else(|_| "cc".into());
    // The packages of `Cargo.lock`, as `name version`, one to a line.
    let lock = fs::read_to_string("Cargo.lock").unwrap_or_default();
    let mut packages = Vec::new();
    let mut name = None;
    for line in lock.lines() {
        if let Some(value) = line.strip_prefix("name = ") {
            name = Some(value.trim_matches('"').to_owned());
        } else if let (Some(value), Some(name)) = (line.strip_prefix("version = "), name.take()) {
            packages.push(format!("{name} {}", value.trim_matches('"')));
        }
    }
    let values = [
        ("RUSTC", first_line(&rustc)),
        ("RUSTFLAGS", rustflags.replace('\x1f', " ")),
        ("CC", first_line(&cc)),
        ("CFLAGS", env::var("CFLAGS").unwrap_or_default()),
        ("PACKAGES", packages.join(";")),
    ];
    for (key, value) in values {
        println!("cargo:rustc-env=FILER_BENCH_{key}={value}");
    }
    println!("cargo:rerun-if-changed=Cargo.lock");
    for key in ["CC", "CFLAGS", "CARGO_ENCODED_RUSTFLAGS"] {
        println!("cargo:rerun-if-env-changed={key}");
    }
}
