// This is free and unencumbered software released into the public domain.

use alloc::{string::String, vec::Vec};

/// The verdict of one specification case.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The driver behaved as specified.
    Pass,

    /// The case was not run, because the [`Profile`](crate::Profile) rules
    /// out its setup.
    Skip(&'static str),

    /// The driver did not behave as specified; the message says how.
    Fail(String),
}

/// The verdict of each case of a [`run`](crate::run), in order.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// Each case's name and verdict.
    pub cases: Vec<(&'static str, Verdict)>,
}

impl Report {
    /// Returns the cases that failed, with their messages.
    pub fn failures(&self) -> impl Iterator<Item = (&'static str, &str)> {
        self.cases
            .iter()
            .filter_map(|(name, verdict)| match verdict {
                Verdict::Fail(message) => Some((*name, message.as_str())),
                _ => None,
            })
    }

    /// Returns `true` if no case failed.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.failures().next().is_none()
    }

    /// Panics, listing every failure, unless no case failed. Meant to end a
    /// driver's test.
    ///
    /// # Panics
    ///
    /// If any case failed.
    #[track_caller]
    pub fn assert_passed(&self) {
        if self.passed() {
            return;
        }
        let mut message = String::from("driver failed the Filer specification:");
        for (name, failure) in self.failures() {
            message.push_str("\n- ");
            message.push_str(name);
            message.push_str(": ");
            message.push_str(failure);
        }
        panic!("{message}");
    }
}

impl core::fmt::Display for Report {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        for (name, verdict) in &self.cases {
            match verdict {
                Verdict::Pass => writeln!(f, "pass {name}")?,
                Verdict::Skip(reason) => writeln!(f, "skip {name}: {reason}")?,
                Verdict::Fail(message) => writeln!(f, "FAIL {name}: {message}")?,
            }
        }
        Ok(())
    }
}
