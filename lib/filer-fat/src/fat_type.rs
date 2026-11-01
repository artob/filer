// This is free and unencumbered software released into the public domain.

/// The variant of FAT a volume uses, decided by its count of data clusters
/// and nothing else, as the FAT specification requires.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum FatType {
    /// Fewer than 4,085 clusters, with 12-bit FAT entries.
    Fat12,

    /// Fewer than 65,525 clusters, with 16-bit FAT entries.
    Fat16,

    /// 65,525 clusters or more, with 28-bit FAT entries.
    Fat32,
}

impl FatType {
    /// The fewest clusters a FAT16 volume has.
    pub const MIN_FAT16_CLUSTERS: u32 = 4085;

    /// The fewest clusters a FAT32 volume has.
    pub const MIN_FAT32_CLUSTERS: u32 = 65525;

    /// The most clusters a FAT32 volume can have: entries from
    /// `0x0FFF_FFF7` up mark bad clusters and chain ends.
    pub const MAX_FAT32_CLUSTERS: u32 = 0x0FFF_FFF5;

    /// Returns the FAT type of a volume with `clusters` data clusters.
    #[must_use]
    pub const fn for_clusters(clusters: u32) -> Self {
        if clusters < Self::MIN_FAT16_CLUSTERS {
            Self::Fat12
        } else if clusters < Self::MIN_FAT32_CLUSTERS {
            Self::Fat16
        } else {
            Self::Fat32
        }
    }

    /// Returns the size of a FAT, in bytes, holding `entries` entries, or
    /// `None` on overflow.
    #[must_use]
    pub const fn table_len(self, entries: u64) -> Option<u64> {
        match self {
            // Two 12-bit entries share three bytes.
            Self::Fat12 => match entries.checked_mul(3) {
                Some(nibbles) => Some(nibbles.div_ceil(2)),
                None => None,
            },
            Self::Fat16 => entries.checked_mul(2),
            Self::Fat32 => entries.checked_mul(4),
        }
    }
}

impl core::fmt::Display for FatType {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(match self {
            Self::Fat12 => "FAT12",
            Self::Fat16 => "FAT16",
            Self::Fat32 => "FAT32",
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decides_by_cluster_count() {
        assert_eq!(FatType::for_clusters(1), FatType::Fat12);
        assert_eq!(FatType::for_clusters(4084), FatType::Fat12);
        assert_eq!(FatType::for_clusters(4085), FatType::Fat16);
        assert_eq!(FatType::for_clusters(65524), FatType::Fat16);
        assert_eq!(FatType::for_clusters(65525), FatType::Fat32);
    }

    #[test]
    fn sizes_tables() {
        assert_eq!(FatType::Fat12.table_len(3), Some(5));
        assert_eq!(FatType::Fat12.table_len(4), Some(6));
        assert_eq!(FatType::Fat16.table_len(3), Some(6));
        assert_eq!(FatType::Fat32.table_len(3), Some(12));
        assert_eq!(FatType::Fat32.table_len(u64::MAX), None);
    }
}
