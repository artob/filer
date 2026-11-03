// This is free and unencumbered software released into the public domain.

/// An archive format.
///
/// Every format is listed whatever this build's features, so code naming
/// one compiles everywhere; [`is_enabled`](Self::is_enabled) says whether
/// this build reads it.
///
/// # Examples
///
/// ```
/// use filer_archive::Format;
///
/// assert_eq!(Format::from_name("release.zip"), Some(Format::Zip));
/// assert_eq!(Format::from_name("hello_1.0_all.deb"), Some(Format::Ar));
/// assert_eq!(Format::from_name("LIBFOO.A"), Some(Format::Ar));
/// assert_eq!(Format::from_name("notes.txt"), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Format {
    /// ar: Debian packages and static libraries, read by `filer-ar`.
    Ar,
    /// cpio: initramfs images and RPM payloads, read by `filer-cpio`.
    Cpio,
    /// TAR, uncompressed, read by `filer-tar`.
    Tar,
    /// ZIP and the formats built on it, read by `filer-zip`.
    Zip,
}

impl Format {
    /// Every format, in order.
    pub const ALL: [Self; 4] = [Self::Ar, Self::Cpio, Self::Tar, Self::Zip];

    /// Returns the format a file name's extension, compared without regard
    /// to ASCII case, says the file is in, if any.
    ///
    /// Only the last extension counts, so a compressed tarball, such as
    /// `release.tar.gz`, has none yet. Names that say nothing of the
    /// format, as an initramfs image's often do (`initrd.img`), need
    /// [`Archive::open_as`](crate::Archive::open_as).
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        let (_, extension) = name.rsplit_once('.')?;
        Self::ALL.into_iter().find(|format| {
            format
                .extensions()
                .iter()
                .any(|known| known.eq_ignore_ascii_case(extension))
        })
    }

    /// Returns the extensions of files in this format, in lowercase.
    #[must_use]
    pub const fn extensions(self) -> &'static [&'static str] {
        match self {
            // Windows' static and import libraries are ar archives too.
            Self::Ar => &["a", "ar", "deb", "lib", "udeb"],
            Self::Cpio => &["cpio"],
            Self::Tar => &["tar"],
            // Java, Android, Python, .NET, and browser packages, e-books,
            // and office documents.
            Self::Zip => &[
                "zip", "jar", "war", "ear", "apk", "aar", "whl", "nupkg", "vsix", "xpi", "epub",
                "docx", "xlsx", "pptx", "odt", "ods", "odp",
            ],
        }
    }

    /// Returns whether this build reads the format: whether its feature,
    /// named as the format is (`ar`, `cpio`, `tar`, or `zip`), is enabled.
    #[must_use]
    pub const fn is_enabled(self) -> bool {
        match self {
            Self::Ar => cfg!(feature = "ar"),
            Self::Cpio => cfg!(feature = "cpio"),
            Self::Tar => cfg!(feature = "tar"),
            Self::Zip => cfg!(feature = "zip"),
        }
    }

    /// Returns the format's name, in lowercase, as its feature has it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Ar => "ar",
            Self::Cpio => "cpio",
            Self::Tar => "tar",
            Self::Zip => "zip",
        }
    }
}

impl core::fmt::Display for Format {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::string::ToString;

    #[test]
    fn detects_formats_by_extension() {
        for format in Format::ALL {
            for extension in format.extensions() {
                let name = alloc::format!("dir.v2/file.{extension}");
                assert_eq!(Format::from_name(&name), Some(format), "{name}");
                let upper = name.to_ascii_uppercase();
                assert_eq!(Format::from_name(&upper), Some(format), "{upper}");
            }
        }
        for name in [
            "",
            "zip",
            "archive",
            "release.tar.gz",
            "initrd.img",
            "a.zip/b",
        ] {
            assert_eq!(Format::from_name(name), None, "{name}");
        }
    }

    #[test]
    fn names_each_format_once() {
        let mut all: alloc::vec::Vec<&str> = Format::ALL
            .iter()
            .flat_map(|format| format.extensions())
            .copied()
            .collect();
        let count = all.len();
        all.sort_unstable();
        all.dedup();
        assert_eq!(all.len(), count, "an extension names two formats");
        assert_eq!(Format::Zip.to_string(), "zip");
    }
}
