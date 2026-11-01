// This is free and unencumbered software released into the public domain.

use crate::{
    Completion, DirCookie, Driver, DynDriver, Entry, Error, Handle, Op, OpKind, Output,
    dir::unexpected,
};
use alloc::{collections::VecDeque, sync::Arc};

/// How many entries [`ReadDir`] asks the driver for at a time.
const BATCH: usize = 256;

/// The entries of a directory, returned by
/// [`Dir::entries`](crate::Dir::entries).
///
/// Entries come in no particular order, never include `.` or `..`, and
/// include names that are not valid UTF-8 (as
/// [`EntryName::Foreign`](crate::EntryName::Foreign)), so a listing has no
/// holes. They are fetched from the driver in batches. Entries added or
/// removed during the listing may or may not appear; others appear exactly
/// once.
///
/// After an error, the listing ends.
///
/// # Examples
///
/// ```
/// # use filer_api::{Dir, Driver};
/// # async fn example<D: Driver>(dir: Dir<D>) -> filer_api::Result<()> {
/// let mut entries = dir.entries()?;
/// while let Some(entry) = entries.next().await {
///     let entry = entry?;
///     println!("{}", entry.name);
/// }
/// # Ok(())
/// # }
/// ```
pub struct ReadDir<D: Driver = DynDriver> {
    driver: D,
    handle: Handle,
    label: Option<Arc<str>>,
    batch: VecDeque<Entry>,
    /// Where the next batch starts; `None` before the first batch.
    cookie: Option<DirCookie>,
    /// Whether the driver has no more entries, or an error ended the
    /// listing.
    done: bool,
}

impl<D: Driver> ReadDir<D> {
    pub(crate) fn new(driver: D, handle: Handle, label: Option<Arc<str>>) -> Self {
        Self {
            driver,
            handle,
            label,
            batch: VecDeque::new(),
            cookie: None,
            done: false,
        }
    }

    /// Returns the next entry, or `None` at the end of the listing.
    ///
    /// # Errors
    ///
    /// Any error from the driver, after which the listing ends.
    pub async fn next(&mut self) -> Option<crate::Result<Entry>> {
        loop {
            if let Some(entry) = self.batch.pop_front() {
                return Some(Ok(entry));
            }
            if self.done {
                return None;
            }
            if let Err(error) = self.fetch().await {
                self.done = true;
                return Some(Err(error));
            }
        }
    }

    async fn fetch(&mut self) -> crate::Result<()> {
        let op = Op::ReadDir {
            dir: self.handle.clone(),
            cookie: self.cookie,
            max: BATCH,
        };
        let Completion { result, .. } = self.driver.submit(op).await;
        let output = result.map_err(|error| self.context(error))?;
        let Output::Entries { entries, next } = output else {
            return Err(unexpected(output, OpKind::ReadDir));
        };
        self.batch.extend(entries);
        self.cookie = next;
        self.done = next.is_none();
        Ok(())
    }

    fn context(&self, error: Error) -> Error {
        match &self.label {
            Some(label) => error.with_label(label),
            None => error,
        }
    }
}

impl<D: Driver> core::fmt::Debug for ReadDir<D> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ReadDir")
            .field("handle", &self.handle)
            .field("buffered", &self.batch.len())
            .field("done", &self.done)
            .finish_non_exhaustive()
    }
}
