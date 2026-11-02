// This is free and unencumbered software released into the public domain.

use std::{fs::File, sync::Arc};

/// The payload of a directory handle.
#[derive(Debug)]
pub(crate) struct DirPayload {
    /// The directory, opened to list and traverse it and to read its
    /// attributes.
    #[allow(dead_code, reason = "read by relative opens (WIN-02)")]
    pub(crate) file: Arc<File>,
}

impl DirPayload {
    pub(crate) const fn new(file: Arc<File>) -> Self {
        Self { file }
    }
}
