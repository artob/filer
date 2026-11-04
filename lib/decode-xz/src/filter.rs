// This is free and unencumbered software released into the public domain.

//! The filters an xz block's data may pass through before LZMA2: Delta,
//! from the `.xz` file format specification, and the branch converters,
//! ported from the LZMA SDK, which turn the absolute addresses that the
//! encoder made of relative branches, calls, and jumps back into relative
//! ones.

/// What a filter is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Kind {
    Delta(u8),
    X86,
    PowerPc,
    Ia64,
    Arm,
    ArmThumb,
    Sparc,
    Arm64,
    RiscV,
}

impl Kind {
    /// Returns the branch converter of filter `id`, if it is one.
    pub(crate) const fn branch(id: u64) -> Option<Self> {
        Some(match id {
            0x04 => Self::X86,
            0x05 => Self::PowerPc,
            0x06 => Self::Ia64,
            0x07 => Self::Arm,
            0x08 => Self::ArmThumb,
            0x09 => Self::Sparc,
            0x0A => Self::Arm64,
            0x0B => Self::RiscV,
            _ => return None,
        })
    }

    /// Returns the alignment of a branch converter's start offset.
    pub(crate) const fn alignment(self) -> u32 {
        match self {
            Self::Delta(_) | Self::X86 => 1,
            Self::ArmThumb | Self::RiscV => 2,
            Self::PowerPc | Self::Arm | Self::Sparc | Self::Arm64 => 4,
            Self::Ia64 => 16,
        }
    }
}

/// A filter, decoding, with what it carries from one piece of its input to
/// the next.
#[derive(Clone, Debug)]
pub(crate) struct Filter {
    kind: Kind,
    /// Where the next byte is, from the start offset, as branches are
    /// relative to.
    ip: u32,
    /// The x86 converter's mask of the recent bytes that began calls.
    state: u32,
    /// Delta's latest bytes, in a ring indexed by `pos`.
    history: [u8; 256],
    pos: u8,
}

impl Filter {
    /// Returns a filter of `kind`, starting at `start`.
    pub(crate) const fn new(kind: Kind, start: u32) -> Self {
        Self {
            kind,
            ip: start,
            state: 0,
            history: [0; 256],
            pos: 0,
        }
    }

    /// Converts what it can of `data` in place, and returns how much: all
    /// of it, at the end of the data, and otherwise up to the last whole
    /// instruction it may convert, the rest to come again with more.
    pub(crate) fn convert(&mut self, data: &mut [u8], end: bool) -> usize {
        let done = match self.kind {
            Kind::Delta(distance) => self.delta(data, distance),
            Kind::X86 => x86(data, self.ip, &mut self.state),
            Kind::PowerPc => power_pc(data, self.ip),
            Kind::Ia64 => ia64(data, self.ip),
            Kind::Arm => arm(data, self.ip),
            Kind::ArmThumb => arm_thumb(data, self.ip),
            Kind::Sparc => sparc(data, self.ip),
            Kind::Arm64 => arm64(data, self.ip),
            Kind::RiscV => risc_v(data, self.ip),
        };
        // What it could not convert at the end stays as it is.
        let done = if end {
            data.len()
        } else {
            done.min(data.len())
        };
        self.ip = self
            .ip
            .wrapping_add(u32::try_from(done).unwrap_or(u32::MAX));
        done
    }

    /// Delta (§5.3.3): each byte adds the one `distance` back, plus one.
    fn delta(&mut self, data: &mut [u8], distance: u8) -> usize {
        let distance = distance.wrapping_add(1);
        for byte in data.iter_mut() {
            let back = self
                .history
                .get(usize::from(distance.wrapping_add(self.pos)))
                .copied()
                .unwrap_or_default();
            *byte = byte.wrapping_add(back);
            if let Some(slot) = self.history.get_mut(usize::from(self.pos)) {
                *slot = *byte;
            }
            self.pos = self.pos.wrapping_sub(1);
        }
        data.len()
    }
}

/// Reads the little-endian word at `at`, if whole.
fn le32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..)?.first_chunk::<4>()?;
    Some(u32::from_le_bytes(*bytes))
}

/// Writes `value` as the little-endian word at `at`, if room.
fn set_le32(data: &mut [u8], at: usize, value: u32) {
    if let Some(bytes) = data
        .get_mut(at..)
        .and_then(|rest| rest.first_chunk_mut::<4>())
    {
        *bytes = value.to_le_bytes();
    }
}

/// Reads the big-endian word at `at`, if whole.
fn be32(data: &[u8], at: usize) -> Option<u32> {
    let bytes = data.get(at..)?.first_chunk::<4>()?;
    Some(u32::from_be_bytes(*bytes))
}

/// Writes `value` as the big-endian word at `at`, if room.
fn set_be32(data: &mut [u8], at: usize, value: u32) {
    if let Some(bytes) = data
        .get_mut(at..)
        .and_then(|rest| rest.first_chunk_mut::<4>())
    {
        *bytes = value.to_be_bytes();
    }
}

/// Returns `i` as the 32 bits that addresses wrap in.
fn offset(i: usize) -> u32 {
    let [a, b, c, d, ..] = i.to_le_bytes();
    u32::from_le_bytes([a, b, c, d])
}

/// Returns the byte at `at`, or zero past the end.
fn byte(data: &[u8], at: usize) -> u8 {
    data.get(at).copied().unwrap_or_default()
}

/// Returns whether `b` may be the high byte of a call's offset: all
/// zeros or all ones.
const fn x86_high(b: u8) -> bool {
    b == 0 || b == 0xFF
}

/// x86 (BCJ): CALL and JMP, `E8` and `E9`, with 32-bit offsets, telling
/// opcodes from the operands of earlier ones by `state`, a mask of the
/// recent bytes that began them.
fn x86(data: &mut [u8], ip: u32, state: &mut u32) -> usize {
    const ALLOWED: [bool; 8] = [true, true, true, false, true, false, false, false];
    const BIT: [u32; 8] = [0, 1, 2, 2, 3, 3, 3, 3];
    let size = data.len();
    if size < 5 {
        return 0;
    }
    let ip = ip.wrapping_add(5);
    let limit = size.wrapping_sub(4);
    let mut mask = *state & 7;
    let (mut pos, mut previous) = (0_usize, usize::MAX);
    loop {
        while pos < limit && byte(data, pos) & 0xFE != 0xE8 {
            pos = pos.wrapping_add(1);
        }
        if pos >= limit {
            break;
        }
        let since = pos.wrapping_sub(previous);
        if since > 3 {
            mask = 0;
        } else {
            mask = mask.wrapping_shl(offset(since).wrapping_sub(1)) & 7;
            if mask != 0 {
                let index = usize::try_from(mask).unwrap_or_default();
                let bit = BIT.get(index).copied().unwrap_or_default();
                let bit = usize::try_from(bit).unwrap_or_default();
                let b = byte(data, pos.wrapping_add(4).wrapping_sub(bit));
                if !ALLOWED.get(index).copied().unwrap_or_default() || x86_high(b) {
                    previous = pos;
                    mask = (mask.wrapping_shl(1) & 7) | 1;
                    pos = pos.wrapping_add(1);
                    continue;
                }
            }
        }
        previous = pos;
        if x86_high(byte(data, pos.wrapping_add(4))) {
            let mut src = le32(data, pos.wrapping_add(1)).unwrap_or_default();
            let mut dest;
            loop {
                dest = src.wrapping_sub(ip.wrapping_add(offset(pos)));
                if mask == 0 {
                    break;
                }
                let index = usize::try_from(mask).unwrap_or_default();
                let bits = BIT.get(index).copied().unwrap_or_default().wrapping_mul(8);
                let [b, ..] = dest.wrapping_shr(24_u32.wrapping_sub(bits)).to_le_bytes();
                if !x86_high(b) {
                    break;
                }
                src = dest
                    ^ 1_u32
                        .wrapping_shl(32_u32.wrapping_sub(bits))
                        .wrapping_sub(1);
            }
            // The high byte all of bit 24.
            let high = !((dest >> 24) & 1).wrapping_sub(1);
            let dest = (dest & 0x00FF_FFFF) | (high << 24);
            set_le32(data, pos.wrapping_add(1), dest);
            pos = pos.wrapping_add(5);
        } else {
            mask = (mask.wrapping_shl(1) & 7) | 1;
            pos = pos.wrapping_add(1);
        }
    }
    let since = pos.wrapping_sub(previous);
    *state = if since > 3 {
        0
    } else {
        mask.wrapping_shl(offset(since).wrapping_sub(1)) & 7
    };
    pos
}

/// ARM: BL, 24-bit word offsets, from 8 bytes ahead.
fn arm(data: &mut [u8], ip: u32) -> usize {
    let mut i = 0;
    let ip = ip.wrapping_add(8);
    while let Some(word) = le32(data, i) {
        if word >> 24 == 0xEB {
            let src = (word & 0x00FF_FFFF) << 2;
            let dest = src.wrapping_sub(ip.wrapping_add(offset(i))) >> 2;
            set_le32(data, i, (word & 0xFF00_0000) | (dest & 0x00FF_FFFF));
        }
        i = i.wrapping_add(4);
    }
    i
}

/// ARM Thumb: BL as two halfwords, 22-bit halfword offsets, from 4 bytes
/// ahead.
fn arm_thumb(data: &mut [u8], ip: u32) -> usize {
    let mut i = 0_usize;
    let ip = ip.wrapping_add(4);
    while i.wrapping_add(4) <= data.len() {
        let (b0, b1, b2, b3) = (
            byte(data, i),
            byte(data, i.wrapping_add(1)),
            byte(data, i.wrapping_add(2)),
            byte(data, i.wrapping_add(3)),
        );
        if b1 & 0xF8 == 0xF0 && b3 & 0xF8 == 0xF8 {
            let src = (u32::from(b1 & 7) << 19)
                | (u32::from(b0) << 11)
                | (u32::from(b3 & 7) << 8)
                | u32::from(b2);
            let dest = (src << 1).wrapping_sub(ip.wrapping_add(offset(i))) >> 1;
            // Bits 11 to 18, 19 to 21, 0 to 7, and 8 to 10.
            let [low, ..] = dest.to_le_bytes();
            let [high, ..] = (dest >> 11).to_le_bytes();
            let [top, ..] = ((dest >> 19) & 7).to_le_bytes();
            let [mid, ..] = ((dest >> 8) & 7).to_le_bytes();
            let bytes = [high, 0xF0 | top, low, 0xF8 | mid];
            if let Some(to) = data.get_mut(i..i.wrapping_add(4)) {
                to.copy_from_slice(&bytes);
            }
            i = i.wrapping_add(2);
        }
        i = i.wrapping_add(2);
    }
    i
}

/// PowerPC: `bl`, big-endian, 24-bit word offsets.
fn power_pc(data: &mut [u8], ip: u32) -> usize {
    let mut i = 0;
    while let Some(word) = be32(data, i) {
        if word & 0xFC00_0003 == 0x4800_0001 {
            let src = word & 0x03FF_FFFC;
            let dest = src.wrapping_sub(ip.wrapping_add(offset(i)));
            set_be32(data, i, 0x4800_0000 | (dest & 0x03FF_FFFF) | (word & 3));
        }
        i = i.wrapping_add(4);
    }
    i
}

/// SPARC: `call`, big-endian, 30-bit word offsets that are in fact 22-bit.
fn sparc(data: &mut [u8], ip: u32) -> usize {
    let mut i = 0;
    while let Some(word) = be32(data, i) {
        let [b0, b1, ..] = word.to_be_bytes();
        if (b0 == 0x40 && b1 & 0xC0 == 0) || (b0 == 0x7F && b1 & 0xC0 == 0xC0) {
            let src = word << 2;
            let dest = src.wrapping_sub(ip.wrapping_add(offset(i))) >> 2;
            let sign = 0_u32.wrapping_sub((dest >> 22) & 1);
            let dest = ((sign << 22) & 0x3FFF_FFFF) | (dest & 0x003F_FFFF) | 0x4000_0000;
            set_be32(data, i, dest);
        }
        i = i.wrapping_add(4);
    }
    i
}

/// IA-64: in each bundle of 16 bytes, the slots its template says may
/// branch, of 41 bits each from bit 5, with 21-bit offsets of bundles.
fn ia64(data: &mut [u8], ip: u32) -> usize {
    const BRANCHES: [u32; 32] = [
        0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 4, 4, 6, 6, 0, 0, 7, 7, 4, 4, 0, 0, 4, 4,
        0, 0,
    ];
    let mut i = 0_usize;
    while let Some(bundle) = data.get_mut(i..i.wrapping_add(16)) {
        let template = usize::from(byte(bundle, 0) & 0x1F);
        let mask = BRANCHES.get(template).copied().unwrap_or_default();
        for slot in 0..3_u32 {
            if (mask >> slot) & 1 == 0 {
                continue;
            }
            let bit = slot.wrapping_mul(41).wrapping_add(5);
            let at = usize::try_from(bit >> 3).unwrap_or_default();
            let shift = bit & 7;
            // The six bytes the slot's 41 bits lie in.
            let mut bytes = [0_u8; 8];
            if let (Some(to), Some(from)) = (bytes.get_mut(..6), bundle.get(at..at.wrapping_add(6)))
            {
                to.copy_from_slice(from);
            }
            let instruction = u64::from_le_bytes(bytes);
            let normal = instruction >> shift;
            if (normal >> 37) & 0xF == 0x5 && (normal >> 9) & 0x7 == 0 {
                let src = u32::try_from((normal >> 13) & 0xF_FFFF).unwrap_or_default()
                    | (u32::try_from((normal >> 36) & 1).unwrap_or_default() << 20);
                let dest = (src << 4).wrapping_sub(ip.wrapping_add(offset(i))) >> 4;
                let mut normal = normal & !(0x008F_FFFF_u64 << 13);
                normal |= u64::from(dest & 0xF_FFFF) << 13;
                normal |= u64::from(dest & 0x10_0000) << (36 - 20);
                let low = instruction & ((1_u64 << shift).wrapping_sub(1));
                let instruction = low | (normal << shift);
                let bytes = instruction.to_le_bytes();
                if let (Some(to), Some(from)) =
                    (bundle.get_mut(at..at.wrapping_add(6)), bytes.get(..6))
                {
                    to.copy_from_slice(from);
                }
            }
        }
        i = i.wrapping_add(16);
    }
    i
}

/// ARM64: BL, with 26-bit word offsets, and ADRP, with 21-bit page
/// offsets, of which only those within ±512 MiB are converted.
fn arm64(data: &mut [u8], ip: u32) -> usize {
    const FLAG: u32 = 1 << (24 - 4);
    const MASK: u32 = (1 << 24) - (FLAG << 1);
    let mut i = 0;
    while let Some(word) = le32(data, i) {
        let pc = ip.wrapping_add(offset(i));
        if word.wrapping_sub(0x9400_0000) & 0xFC00_0000 == 0 {
            let v = word.wrapping_sub(pc >> 2);
            set_le32(data, i, (v & 0x03FF_FFFF) | 0x9400_0000);
        } else {
            let v = word.wrapping_sub(0x9000_0000);
            if v & 0x9F00_0000 == 0 {
                let v = v.wrapping_add(FLAG);
                if v & MASK == 0 {
                    let z = (v & 0xFFFF_FFE0) | (v >> 26);
                    let c = (pc >> (12 - 3)) & !7;
                    let z = z.wrapping_sub(c);
                    let mut out = (v & 0x1F) | 0x9000_0000;
                    out |= z << 26;
                    out |= 0x00FF_FFE0 & (z & ((FLAG << 1) - 1)).wrapping_sub(FLAG);
                    set_le32(data, i, out);
                }
            }
        }
        i = i.wrapping_add(4);
    }
    i
}

/// RISC-V: JAL, and AUIPC with the instruction after it, which the
/// encoder made easier to compress, as the LZMA SDK's decoder undoes.
fn risc_v(data: &mut [u8], ip: u32) -> usize {
    let size = data.len() & !1;
    if size <= 6 {
        return 0;
    }
    let limit = size.wrapping_sub(6);
    let load16 = |data: &[u8], at: usize| {
        u32::from(u16::from_le_bytes([
            byte(data, at),
            byte(data, at.wrapping_add(1)),
        ]))
    };
    let mut p = 0_usize;
    loop {
        // Find a JAL or an AUIPC, at an even offset.
        let mut a;
        loop {
            if p >= limit {
                return p;
            }
            a = (load16(data, p) ^ 0x10).wrapping_add(1);
            if a & 0x77 == 0 {
                break;
            }
            a = (load16(data, p.wrapping_add(2)) ^ 0x10).wrapping_add(1);
            p = p.wrapping_add(4);
            if a & 0x77 == 0 {
                p = p.wrapping_sub(2);
                if p >= limit {
                    return p;
                }
                break;
            }
        }
        let pc = ip.wrapping_add(offset(p));
        if a & 8 == 0 {
            // JAL.
            a = a.wrapping_sub(0x100 - 0x7F);
            if a & 0xD80 != 0 {
                p = p.wrapping_add(2);
                continue;
            }
            let old = a.wrapping_add(0xEF - 0x7F) & 0xFFF;
            let mut v = u32::from(byte(data, p.wrapping_add(3))) << 1
                | u32::from(byte(data, p.wrapping_add(2))) << 9
                | ((a & 0xF000) << 5);
            v = v.wrapping_sub(pc);
            let a = old
                | (v << 11 & 1 << 31)
                | (v << 20 & 0x3FF << 21)
                | (v << 9 & 1 << 20)
                | (v & 0xFF << 12);
            set_le32(data, p, a);
            p = p.wrapping_add(4);
            continue;
        }
        // AUIPC.
        let v = a;
        let a = le32(data, p).unwrap_or_default();
        if v & 0xE80 == 0 {
            // Of x0 or x2: the encoder's form of an AUIPC it changed.
            let r = a >> 27;
            let check = v.wrapping_sub((3 << 12) | (2 << 7) | 8) << 18;
            if check < r & 0x1D {
                let b = be32(data, p.wrapping_add(4))
                    .unwrap_or_default()
                    .wrapping_sub(pc);
                let first = (r << 7)
                    .wrapping_add(0x17)
                    .wrapping_add(b.wrapping_add(0x800) & 0xFFFF_F000);
                let second = (a >> 12) | (b << 20);
                set_le32(data, p, first);
                set_le32(data, p.wrapping_add(4), second);
                p = p.wrapping_add(8);
            } else {
                p = p.wrapping_add(4);
            }
        } else {
            let b = le32(data, p.wrapping_add(4)).unwrap_or_default();
            let check = ((b.wrapping_sub(3) ^ (v << 8)) & (0xF_8000 + 3)) == 0;
            if check {
                let second = (a & 0xFFFF_F000) | (b >> 20);
                let first = (b << 12) | (0x17 + (2 << 7));
                set_le32(data, p, first);
                set_le32(data, p.wrapping_add(4), second);
                p = p.wrapping_add(8);
            } else {
                p = p.wrapping_add(6);
            }
        }
    }
}
