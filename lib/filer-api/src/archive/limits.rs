// This is free and unencumbered software released into the public domain.

/// Safety limits applied while indexing an archive.
///
/// Archives are untrusted input; the defaults are strict and suit typical
/// source and release archives. Exceeding a limit fails the open with
/// [`LimitExceeded`](crate::ErrorKind::LimitExceeded). The size limits
/// apply to formats that compress members, such as ZIP, and are checked
/// against the sizes the archive declares, which also bound decoding.
///
/// # Examples
///
/// ```
/// use filer_api::archive::Limits;
///
/// let limits = Limits::default().with_max_entries(1_000_000);
/// assert_eq!(limits.max_entries, 1_000_000);
///
/// // 1 GiB from 1 MiB is a bomb; 1 MiB from almost nothing is exempt.
/// assert!(limits.exceeds_ratio(1 << 30, 1 << 20));
/// assert!(!limits.exceeds_ratio(Limits::RATIO_EXEMPT, 1));
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct Limits {
    /// The most members the index may hold, which bounds its memory.
    /// Default: 100,000.
    pub max_entries: usize,

    /// The longest member name or link target, in bytes. Default: 4,096.
    pub max_path_len: usize,

    /// The largest extended header record (such as a TAR PAX header or GNU
    /// long name), in bytes. Default: 64 KiB.
    pub max_extension_len: u64,

    /// The largest total of the members' decoded sizes, which bounds what
    /// extracting the archive can write. Default: 16 GiB.
    pub max_total_size: u64,

    /// The largest ratio of a member's decoded size to its packed size,
    /// against decompression bombs; members of up to
    /// [`RATIO_EXEMPT`](Self::RATIO_EXEMPT) bytes are exempt, since small
    /// files of repeated bytes are common and harmless. Default: 100.
    pub max_ratio: u64,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_entries: 100_000,
            max_path_len: 4096,
            max_extension_len: 64 * 1024,
            max_total_size: 16 << 30,
            max_ratio: 100,
        }
    }
}

impl Limits {
    /// The decoded size up to which members are exempt from
    /// [`max_ratio`](Self::max_ratio): 1 MiB.
    pub const RATIO_EXEMPT: u64 = 1 << 20;

    /// Returns whether a member of `size` bytes decoded, `packed` bytes in
    /// the archive, exceeds [`max_ratio`](Self::max_ratio).
    #[must_use]
    pub const fn exceeds_ratio(&self, size: u64, packed: u64) -> bool {
        size > Self::RATIO_EXEMPT && size > packed.saturating_mul(self.max_ratio)
    }

    /// Sets the most members the index may hold.
    #[must_use]
    pub const fn with_max_entries(mut self, max_entries: usize) -> Self {
        self.max_entries = max_entries;
        self
    }

    /// Sets the longest member name or link target, in bytes.
    #[must_use]
    pub const fn with_max_path_len(mut self, max_path_len: usize) -> Self {
        self.max_path_len = max_path_len;
        self
    }

    /// Sets the largest extended header record, in bytes.
    #[must_use]
    pub const fn with_max_extension_len(mut self, max_extension_len: u64) -> Self {
        self.max_extension_len = max_extension_len;
        self
    }

    /// Sets the largest total of the members' decoded sizes.
    #[must_use]
    pub const fn with_max_total_size(mut self, max_total_size: u64) -> Self {
        self.max_total_size = max_total_size;
        self
    }

    /// Sets the largest ratio of a member's decoded to packed size.
    #[must_use]
    pub const fn with_max_ratio(mut self, max_ratio: u64) -> Self {
        self.max_ratio = max_ratio;
        self
    }
}
