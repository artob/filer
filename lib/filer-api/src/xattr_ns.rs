// This is free and unencumbered software released into the public domain.

/// The namespace of an extended attribute, as on Linux.
///
/// Namespaces map natively on Linux and FreeBSD. macOS has one flat
/// namespace, which drivers map to [`User`](Self::User). Drivers for
/// filesystems with named data streams (NTFS) map those streams to
/// [`User`](Self::User), as ntfs-3g does.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum XattrNs {
    /// Attributes any user with access to the file may use (`user.`).
    User,

    /// Attributes only privileged processes may use (`trusted.`).
    Trusted,

    /// Attributes of security modules, such as SELinux labels and file
    /// capabilities (`security.`).
    Security,

    /// Attributes the filesystem defines, such as POSIX ACLs
    /// (`system.`).
    System,
}

impl XattrNs {
    /// Every namespace.
    pub const ALL: [Self; 4] = [Self::User, Self::Trusted, Self::Security, Self::System];

    /// Returns the namespace's prefix, without the separating `.`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::Trusted => "trusted",
            Self::Security => "security",
            Self::System => "system",
        }
    }

    /// Returns the namespace with the prefix `prefix`, without the
    /// separating `.`.
    #[must_use]
    pub fn from_prefix(prefix: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|ns| ns.as_str() == prefix)
    }
}

impl core::fmt::Display for XattrNs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_prefixes() {
        for ns in XattrNs::ALL {
            assert_eq!(XattrNs::from_prefix(ns.as_str()), Some(ns));
        }
        assert_eq!(XattrNs::from_prefix("User"), None);
        assert_eq!(XattrNs::from_prefix(""), None);
    }
}
