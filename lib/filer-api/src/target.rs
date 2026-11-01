// This is free and unencumbered software released into the public domain.

use crate::{EntryName, Handle, Resolve};

/// What an operation such as [`Op::Stat`](crate::Op::Stat) acts on.
#[derive(Clone, Debug)]
pub enum Target {
    /// An open file or directory.
    Handle(Handle),

    /// One entry of a directory, by name, without opening it.
    Entry {
        /// The directory containing the entry.
        at: Handle,

        /// The entry's name; a foreign name only from the same driver.
        name: EntryName,

        /// Whether to follow the entry if it is a symlink.
        follow: bool,

        /// How to follow it: the policy of the directory handle `at` came
        /// from, so that a single-entry operation never loosens it.
        resolve: Resolve,
    },
}
