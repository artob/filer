// This is free and unencumbered software released into the public domain.

bitflags::bitflags! {
    /// The [`Metadata`](crate::Metadata) fields a caller wants.
    ///
    /// Like Linux `statx`, metadata is requested field by field, because
    /// fields cost differently: birth time may need an extra lookup, and on
    /// network filesystems every unneeded field can be a round trip. A
    /// driver may return more than was asked for; fields it cannot provide
    /// stay `None`.
    ///
    /// More fields (permissions, allocated size, ownership) are added with the operations that need them; existing bit values never
    /// change.
    #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
    pub struct MetaMask: u32 {
        /// [`Metadata::kind`](crate::Metadata::kind).
        const KIND = 1 << 0;
        /// [`Metadata::len`](crate::Metadata::len).
        const LEN = 1 << 2;
        /// [`Metadata::modified`](crate::Metadata::modified).
        const MTIME = 1 << 4;
        /// [`Metadata::changed`](crate::Metadata::changed).
        const CTIME = 1 << 5;
        /// [`Metadata::created`](crate::Metadata::created).
        const BTIME = 1 << 6;
        /// [`Metadata::accessed`](crate::Metadata::accessed).
        const ATIME = 1 << 7;
        /// [`Metadata::id`](crate::Metadata::id).
        const ID = 1 << 8;
        /// [`Metadata::attrs`](crate::Metadata::attrs).
        const ATTRS = 1 << 11;
    }
}
