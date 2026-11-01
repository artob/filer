// This is free and unencumbered software released into the public domain.

use super::Name;

/// An iterator over the [`Name`]s in a [`RelPath`](super::RelPath).
///
/// Returned by [`RelPath::components`](super::RelPath::components). The
/// empty path has no components.
#[derive(Clone, Debug)]
pub struct Components<'a> {
    rest: &'a str,
}

impl<'a> Components<'a> {
    pub(super) const fn new(path: &'a str) -> Self {
        Self { rest: path }
    }
}

impl<'a> Iterator for Components<'a> {
    type Item = &'a Name;

    fn next(&mut self) -> Option<&'a Name> {
        if self.rest.is_empty() {
            return None;
        }
        let (first, rest) = self.rest.split_once('/').unwrap_or((self.rest, ""));
        self.rest = rest;
        Some(Name::from_str_unchecked(first))
    }
}

impl<'a> DoubleEndedIterator for Components<'a> {
    fn next_back(&mut self) -> Option<&'a Name> {
        if self.rest.is_empty() {
            return None;
        }
        let (rest, last) = self.rest.rsplit_once('/').unwrap_or(("", self.rest));
        self.rest = rest;
        Some(Name::from_str_unchecked(last))
    }
}

impl core::iter::FusedIterator for Components<'_> {}
