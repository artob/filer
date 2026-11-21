// This is free and unencumbered software released into the public domain.

//! Every decoder the harness measures, ours and the counterparts, driven
//! through one interface: a whole stream into a buffer, or a stream given
//! and taken in pieces of a size. Counterparts are run, never read.

use std::io::Read;

/// Why a decoder failed, as text, as each crate's errors differ.
pub type Failure = String;

/// Decodes `input` into `output`, given and taken in pieces of at most
/// the size it is given, or whole for `usize::MAX`; returns how many bytes
/// it wrote.
pub type Decode = fn(&[u8], &mut [u8], usize) -> Result<usize, Failure>;

/// A decoder of a stream given and taken in pieces that offers
/// checkpoints, as `checkpoints` measures them: one of ours, or a
/// counterpart's that copies its state.
pub trait Resumable {
    /// Decodes what it can of `input` into `output`, and returns how much
    /// it consumed and produced: neither once it has ended.
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Failure>;

    /// Returns a decoder that goes on from where this one is, if it offers
    /// one here.
    fn checkpoint(&self) -> Option<Box<dyn Resumable>>;

    /// Returns how many of the bytes it consumed its checkpoint must be
    /// given again.
    fn buffered(&self) -> usize;
}

/// Makes a decoder that offers checkpoints.
pub type Resume = fn() -> Result<Box<dyn Resumable>, Failure>;

/// A decoder, ours or a counterpart.
pub struct Decoder {
    /// Its name in tables.
    pub name: &'static str,
    /// The crate it is, as `Cargo.toml` names it, and its language.
    pub krate: &'static str,
    pub language: &'static str,
    pub decode: Decode,
    /// The size of the value that its caller holds as it decodes: the
    /// decoder itself, or a handle to it on the heap.
    pub value: usize,
    /// Whether it decodes only whole streams, held in memory, and so is
    /// left out where they are given and taken in pieces.
    pub whole: bool,
    /// It as a decoder that offers checkpoints, if it does.
    pub resumable: Option<Resume>,
}

/// A codec, and its decoders, ours first, then the reference.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Codec {
    Bzip2,
    Deflate,
    Lzma,
    Xz,
    Zstd,
}

impl Codec {
    pub const ALL: [Self; 5] = [Self::Bzip2, Self::Deflate, Self::Lzma, Self::Xz, Self::Zstd];

    pub fn name(self) -> &'static str {
        match self {
            Self::Bzip2 => "bzip2",
            Self::Deflate => "deflate",
            Self::Lzma => "lzma",
            Self::Xz => "xz",
            Self::Zstd => "zstd",
        }
    }

    pub fn decoders(self) -> &'static [Decoder] {
        match self {
            Self::Bzip2 => &BZIP2,
            Self::Deflate => &DEFLATE,
            Self::Lzma => &LZMA,
            Self::Xz => &XZ,
            Self::Zstd => &ZSTD,
        }
    }

    /// Its decoders of streams given and taken in pieces of `piece` bytes,
    /// or whole for `usize::MAX`: all of them whole, and in pieces those
    /// that take them so.
    pub fn fed(self, piece: usize) -> Vec<&'static Decoder> {
        let decoders = self.decoders().iter();
        decoders
            .filter(|decoder| piece == usize::MAX || !decoder.whole)
            .collect()
    }
}

const BZIP2: [Decoder; 4] = [
    Decoder {
        name: "ours",
        krate: "decode-bzip2",
        language: "Rust",
        decode: ours_bzip2,
        value: size_of::<decode_bzip2::Decoder>(),
        whole: false,
        resumable: Some(ours_bzip2_resumable),
    },
    Decoder {
        name: "libbzip2",
        krate: "bzip2-sys",
        language: "C",
        decode: libbzip2,
        value: size_of::<bzip2_c::Decompress>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "libbz2-rs-sys",
        krate: "libbz2-rs-sys",
        language: "Rust",
        decode: libbz2_rs,
        value: size_of::<libbz2_rs::Decompress>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "bzip2-rs",
        krate: "bzip2-rs",
        language: "Rust",
        decode: bzip2_rs,
        value: size_of::<bzip2_rs::decoder::Decoder>(),
        whole: false,
        resumable: None,
    },
];

const DEFLATE: [Decoder; 8] = [
    Decoder {
        name: "ours",
        krate: "decode-deflate",
        language: "Rust",
        decode: ours_deflate,
        value: size_of::<decode_deflate::Decoder>(),
        whole: false,
        resumable: Some(ours_deflate_resumable),
    },
    Decoder {
        name: "zlib",
        krate: "libz-sys",
        language: "C",
        decode: zlib::inflate,
        value: size_of::<libz_sys::z_stream>(),
        whole: false,
        resumable: Some(zlib::resumable),
    },
    Decoder {
        name: "zlib-ng",
        krate: "libz-ng-sys",
        language: "C",
        decode: zlib_ng::inflate,
        value: size_of::<libz_ng_sys::z_stream>(),
        whole: false,
        resumable: Some(zlib_ng::resumable),
    },
    Decoder {
        name: "libdeflate",
        krate: "libdeflater",
        language: "C",
        decode: libdeflate,
        value: size_of::<libdeflater::Decompressor>(),
        whole: true,
        resumable: None,
    },
    Decoder {
        name: "zlib-rs",
        krate: "zlib-rs",
        language: "Rust",
        decode: zlib_rs,
        value: size_of::<zlib_rs::Inflate>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "miniz_oxide",
        krate: "miniz_oxide",
        language: "Rust",
        decode: miniz_oxide,
        value: size_of::<Box<miniz_oxide::inflate::stream::InflateState>>(),
        whole: false,
        resumable: Some(miniz_oxide_resumable),
    },
    Decoder {
        name: "zune-inflate",
        krate: "zune-inflate",
        language: "Rust",
        decode: zune_inflate,
        value: size_of::<zune_inflate::DeflateDecoder<'static>>(),
        whole: true,
        resumable: None,
    },
    Decoder {
        name: "yazi",
        krate: "yazi",
        language: "Rust",
        decode: yazi,
        value: size_of::<yazi::Decoder>(),
        whole: false,
        resumable: None,
    },
];

const LZMA: [Decoder; 4] = [
    Decoder {
        name: "ours",
        krate: "decode-lzma",
        language: "Rust",
        decode: ours_lzma,
        value: size_of::<decode_lzma::Decoder>(),
        whole: false,
        resumable: Some(ours_lzma_resumable),
    },
    Decoder {
        name: "liblzma",
        krate: "liblzma",
        language: "C",
        decode: liblzma_lzma,
        value: size_of::<liblzma::stream::Stream>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "lzma-rust2",
        krate: "lzma-rust2",
        language: "Rust",
        decode: lzma_rust2_lzma,
        value: size_of::<lzma_rust2::LzmaStream>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "lzma-rs",
        krate: "lzma-rs",
        language: "Rust",
        decode: lzma_rs_lzma,
        value: size_of::<lzma_rs::decompress::Stream<Room<'static>>>(),
        whole: false,
        resumable: None,
    },
];

const XZ: [Decoder; 5] = [
    Decoder {
        name: "ours",
        krate: "decode-xz",
        language: "Rust",
        decode: ours_xz,
        value: size_of::<decode_xz::Decoder>(),
        whole: false,
        resumable: Some(ours_xz_resumable),
    },
    Decoder {
        name: "liblzma",
        krate: "liblzma",
        language: "C",
        decode: liblzma_xz,
        value: size_of::<liblzma::stream::Stream>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "lzma-rust2",
        krate: "lzma-rust2",
        language: "Rust",
        decode: lzma_rust2_xz,
        value: size_of::<lzma_rust2::XzStream>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "lzma-rs",
        krate: "lzma-rs",
        language: "Rust",
        decode: lzma_rs_xz,
        // A function, whose state is its own.
        value: 0,
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "xz4rust",
        krate: "xz4rust",
        language: "Rust",
        decode: xz4rust,
        value: size_of::<Box<xz4rust::XzDecoder<'static>>>(),
        whole: false,
        resumable: None,
    },
];

const ZSTD: [Decoder; 3] = [
    Decoder {
        name: "ours",
        krate: "decode-zstd",
        language: "Rust",
        decode: ours_zstd,
        value: size_of::<decode_zstd::Decoder>(),
        whole: false,
        resumable: Some(ours_zstd_resumable),
    },
    Decoder {
        name: "libzstd",
        krate: "zstd",
        language: "C",
        decode: libzstd,
        value: size_of::<zstd::stream::raw::Decoder<'static>>(),
        whole: false,
        resumable: None,
    },
    Decoder {
        name: "ruzstd",
        krate: "ruzstd",
        language: "Rust",
        decode: ruzstd,
        value: size_of::<ruzstd::decoding::FrameDecoder>(),
        whole: false,
        resumable: None,
    },
];

/// Drives one of our sans-I/O decoders, `step`, over `input` into
/// `output`, in pieces of at most `piece` bytes each way.
fn sans_io<E: std::fmt::Debug>(
    mut step: impl FnMut(&[u8], &mut [u8]) -> Result<(usize, usize), E>,
    input: &[u8],
    output: &mut [u8],
    piece: usize,
) -> Result<usize, Failure> {
    let (mut read, mut written) = (0_usize, 0_usize);
    loop {
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let (consumed, produced) = step(&input[read..into], &mut output[written..room])
            .map_err(|error| format!("{error:?}"))?;
        (read, written) = (read + consumed, written + produced);
        if (consumed, produced) == (0, 0) {
            return Ok(written);
        }
    }
}

fn ours_bzip2(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut decoder = decode_bzip2::Decoder::new();
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

fn ours_zstd(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut decoder = decode_zstd::Decoder::new();
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

fn ours_deflate(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut decoder = decode_deflate::Decoder::new();
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

fn ours_xz(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut decoder = decode_xz::Decoder::new();
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

/// The length of a `.lzma` file's header: its properties, then the size
/// it decodes to, little-endian, all ones if unknown.
const LZMA_HEADER: usize = 13;

/// Our decoder of the stream that follows `header`.
fn ours_lzma_after(header: &[u8; LZMA_HEADER]) -> Result<decode_lzma::Decoder, Failure> {
    let (properties, size) = header.split_first_chunk::<5>().ok_or("no properties")?;
    let size = u64::from_le_bytes(size.try_into().map_err(|_| "no size")?);
    let size = (size != u64::MAX).then_some(size);
    decode_lzma::Decoder::lzma(*properties, size).map_err(|error| error.to_string())
}

fn ours_lzma(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let (header, input) = input.split_first_chunk().ok_or("no header")?;
    let mut decoder = ours_lzma_after(header)?;
    let written = sans_io(|i, o| decoder.decode(i, o), input, output, piece)?;
    decoder.finish().map_err(|error| error.to_string())?;
    Ok(written)
}

/// Our decoder of a `.lzma` file as `Resumable`, which takes its header
/// first: callers read it, as it is not LZMA's own.
struct OursLzma {
    header: Vec<u8>,
    decoder: Option<decode_lzma::Decoder>,
}

fn ours_lzma_resumable() -> Result<Box<dyn Resumable>, Failure> {
    Ok(Box::new(OursLzma {
        header: Vec::with_capacity(LZMA_HEADER),
        decoder: None,
    }))
}

impl Resumable for OursLzma {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Failure> {
        if let Some(decoder) = &mut self.decoder {
            return decoder
                .decode(input, output)
                .map_err(|error| format!("{error:?}"));
        }
        let len = input.len().min(LZMA_HEADER - self.header.len());
        self.header.extend_from_slice(&input[..len]);
        if let Some(header) = self.header.first_chunk() {
            self.decoder = Some(ours_lzma_after(header)?);
        }
        Ok((len, 0))
    }

    fn checkpoint(&self) -> Option<Box<dyn Resumable>> {
        let decoder = self.decoder.as_ref()?.checkpoint()?;
        Some(Box::new(Self {
            header: Vec::new(),
            decoder: Some(decoder),
        }))
    }

    fn buffered(&self) -> usize {
        self.decoder
            .as_ref()
            .map_or(0, decode_lzma::Decoder::buffered)
    }
}

/// One of our decoders as `Resumable`, by its own `checkpoint` and
/// `buffered`; and `$new`, which makes one.
macro_rules! ours_resumable {
    ($new:ident, $decoder:ty) => {
        fn $new() -> Result<Box<dyn Resumable>, Failure> {
            Ok(Box::new(<$decoder>::new()))
        }

        impl Resumable for $decoder {
            fn decode(
                &mut self,
                input: &[u8],
                output: &mut [u8],
            ) -> Result<(usize, usize), Failure> {
                <$decoder>::decode(self, input, output).map_err(|error| format!("{error:?}"))
            }

            fn checkpoint(&self) -> Option<Box<dyn Resumable>> {
                let resumed = <$decoder>::checkpoint(self)?;
                Some(Box::new(resumed))
            }

            fn buffered(&self) -> usize {
                <$decoder>::buffered(self)
            }
        }
    };
}

ours_resumable!(ours_bzip2_resumable, decode_bzip2::Decoder);
ours_resumable!(ours_deflate_resumable, decode_deflate::Decoder);
ours_resumable!(ours_xz_resumable, decode_xz::Decoder);
ours_resumable!(ours_zstd_resumable, decode_zstd::Decoder);

// C's own allocator, which zlib's streams are given, as the bindings ask
// for one: on Linux with glibc, `memory`'s, which counts.
unsafe extern "C" {
    fn calloc(count: usize, size: usize) -> *mut std::ffi::c_void;
    fn free(pointer: *mut std::ffi::c_void);
}

/// Drives zlib's interface, as a binding of either C library declares it:
/// zlib's own, in `libz-sys`, or zlib-ng's, prefixed, in `libz-ng-sys`.
macro_rules! zlib_interface {
    ($module:ident, $krate:ident) => {
        pub mod $module {
            use super::Failure;
            use $krate as z;

            unsafe extern "C" fn zalloc(_: z::voidpf, items: z::uInt, size: z::uInt) -> z::voidpf {
                unsafe { super::calloc(items as usize, size as usize) }
            }

            unsafe extern "C" fn zfree(_: z::voidpf, address: z::voidpf) {
                unsafe { super::free(address) }
            }

            /// A stream not yet begun, which must stay where it is once it
            /// is, as its state points back at it.
            pub fn stream() -> z::z_stream {
                z::z_stream {
                    next_in: std::ptr::null_mut(),
                    avail_in: 0,
                    total_in: 0,
                    next_out: std::ptr::null_mut(),
                    avail_out: 0,
                    total_out: 0,
                    msg: std::ptr::null_mut(),
                    state: std::ptr::null_mut(),
                    zalloc,
                    zfree,
                    opaque: std::ptr::null_mut(),
                    data_type: 0,
                    adler: 0,
                    reserved: 0,
                }
            }

            /// Decodes raw Deflate, whole in one call that may finish it,
            /// as zlib decodes fastest, or in pieces.
            pub fn inflate(
                input: &[u8],
                output: &mut [u8],
                piece: usize,
            ) -> Result<usize, Failure> {
                let mut stream = stream();
                let size = size_of::<z::z_stream>() as std::ffi::c_int;
                let status = unsafe { z::inflateInit2_(&mut stream, -15, z::zlibVersion(), size) };
                if status != z::Z_OK {
                    return Err(format!("began with {status}"));
                }
                let flush = match piece {
                    usize::MAX => z::Z_FINISH,
                    _ => z::Z_NO_FLUSH,
                };
                let result = loop {
                    let (read, written) = (stream.total_in as usize, stream.total_out as usize);
                    let into = input.len().min(read.saturating_add(piece));
                    let room = output.len().min(written.saturating_add(piece));
                    stream.next_in = input[read..into].as_ptr().cast_mut();
                    stream.avail_in = (into - read) as _;
                    stream.next_out = output[written..room].as_mut_ptr();
                    stream.avail_out = (room - written) as _;
                    let status = unsafe { z::inflate(&mut stream, flush) };
                    let moved =
                        (stream.total_in as usize, stream.total_out as usize) != (read, written);
                    match status {
                        z::Z_STREAM_END => break Ok(stream.total_out as usize),
                        z::Z_OK | z::Z_BUF_ERROR if moved => {},
                        z::Z_OK | z::Z_BUF_ERROR => break Err("cut short".into()),
                        status => break Err(format!("failed with {status}")),
                    }
                };
                unsafe { z::inflateEnd(&mut stream) };
                result
            }

            /// A stream of raw Deflate decoded in pieces, boxed, as its
            /// state points back at it, which offers checkpoints as copies
            /// of its state, by `inflateCopy`.
            pub struct Inflate(Box<z::z_stream>);

            impl Drop for Inflate {
                fn drop(&mut self) {
                    unsafe { z::inflateEnd(&mut *self.0) };
                }
            }

            pub fn resumable() -> Result<Box<dyn super::Resumable>, Failure> {
                let mut stream = Box::new(stream());
                let size = size_of::<z::z_stream>() as std::ffi::c_int;
                let status = unsafe { z::inflateInit2_(&mut *stream, -15, z::zlibVersion(), size) };
                match status {
                    z::Z_OK => Ok(Box::new(Inflate(stream))),
                    status => Err(format!("began with {status}")),
                }
            }

            impl super::Resumable for Inflate {
                fn decode(
                    &mut self,
                    input: &[u8],
                    output: &mut [u8],
                ) -> Result<(usize, usize), Failure> {
                    let stream = &mut *self.0;
                    stream.next_in = input.as_ptr().cast_mut();
                    stream.avail_in = z::uInt::try_from(input.len()).map_err(|e| e.to_string())?;
                    stream.next_out = output.as_mut_ptr();
                    stream.avail_out =
                        z::uInt::try_from(output.len()).map_err(|e| e.to_string())?;
                    let status = unsafe { z::inflate(stream, z::Z_NO_FLUSH) };
                    let consumed = input.len() - stream.avail_in as usize;
                    let produced = output.len() - stream.avail_out as usize;
                    match status {
                        z::Z_OK | z::Z_STREAM_END | z::Z_BUF_ERROR => Ok((consumed, produced)),
                        status => Err(format!("failed with {status}")),
                    }
                }

                fn checkpoint(&self) -> Option<Box<dyn super::Resumable>> {
                    let mut copy = Box::new(stream());
                    let source = std::ptr::from_ref(&*self.0).cast_mut();
                    let status = unsafe { z::inflateCopy(&mut *copy, source) };
                    match status {
                        z::Z_OK => Some(Box::new(Inflate(copy))),
                        _ => None,
                    }
                }

                fn buffered(&self) -> usize {
                    0
                }
            }
        }
    };
}

zlib_interface!(zlib, libz_sys);
zlib_interface!(zlib_ng, libz_ng_sys);

fn libdeflate(input: &[u8], output: &mut [u8], _: usize) -> Result<usize, Failure> {
    let mut decoder = libdeflater::Decompressor::new();
    decoder
        .deflate_decompress(input, output)
        .map_err(|error| format!("{error:?}"))
}

fn zlib_rs(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use zlib_rs::{InflateConfig, InflateFlush, ReturnCode, Status};
    if piece == usize::MAX {
        let config = InflateConfig { window_bits: -15 };
        return match zlib_rs::decompress_slice(output, input, config) {
            (decoded, ReturnCode::Ok) => Ok(decoded.len()),
            (_, code) => Err(format!("{code:?}")),
        };
    }
    let mut decoder = zlib_rs::Inflate::new(false, 15);
    loop {
        let (read, written) = (decoder.total_in() as usize, decoder.total_out() as usize);
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let status = decoder
            .decompress(
                &input[read..into],
                &mut output[written..room],
                InflateFlush::NoFlush,
            )
            .map_err(|error| format!("{error:?}"))?;
        if status == Status::StreamEnd {
            return Ok(decoder.total_out() as usize);
        }
        if (decoder.total_in() as usize, decoder.total_out() as usize) == (read, written) {
            return Err("cut short".into());
        }
    }
}

fn miniz_oxide(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use miniz_oxide::{
        DataFormat, MZFlush, MZStatus,
        inflate::{self, stream::InflateState},
    };
    if piece == usize::MAX {
        let input = std::iter::once(input);
        return inflate::decompress_slice_iter_to_slice(output, input, false, true)
            .map_err(|status| format!("{status:?}"));
    }
    let mut state = InflateState::new_boxed(DataFormat::Raw);
    let (mut read, mut written) = (0_usize, 0_usize);
    loop {
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let result = inflate::stream::inflate(
            &mut state,
            &input[read..into],
            &mut output[written..room],
            MZFlush::None,
        );
        let (consumed, produced) = (result.bytes_consumed, result.bytes_written);
        (read, written) = (read + consumed, written + produced);
        match result.status {
            Ok(MZStatus::StreamEnd) => return Ok(written),
            _ if (consumed, produced) != (0, 0) => {},
            Ok(_) => return Err("cut short".into()),
            Err(error) => return Err(format!("{error:?}")),
        }
    }
}

/// `miniz_oxide`'s state, boxed, which offers checkpoints as clones.
struct MinizOxide(Box<miniz_oxide::inflate::stream::InflateState>);

fn miniz_oxide_resumable() -> Result<Box<dyn Resumable>, Failure> {
    use miniz_oxide::{DataFormat, inflate::stream::InflateState};
    Ok(Box::new(MinizOxide(InflateState::new_boxed(
        DataFormat::Raw,
    ))))
}

impl Resumable for MinizOxide {
    fn decode(&mut self, input: &[u8], output: &mut [u8]) -> Result<(usize, usize), Failure> {
        use miniz_oxide::{MZError, MZFlush, inflate::stream::inflate};
        let result = inflate(&mut self.0, input, output, MZFlush::None);
        match result.status {
            Ok(_) | Err(MZError::Buf) => Ok((result.bytes_consumed, result.bytes_written)),
            Err(error) => Err(format!("{error:?}")),
        }
    }

    fn checkpoint(&self) -> Option<Box<dyn Resumable>> {
        Some(Box::new(Self(self.0.clone())))
    }

    fn buffered(&self) -> usize {
        0
    }
}

/// It decodes into a buffer of its own, which is then copied into the
/// caller's, timed with it.
fn zune_inflate(input: &[u8], output: &mut [u8], _: usize) -> Result<usize, Failure> {
    use zune_inflate::{DeflateDecoder, DeflateOptions};
    let options = DeflateOptions::default()
        .set_size_hint(output.len())
        .set_limit(output.len());
    let decoded = DeflateDecoder::new_with_options(input, options)
        .decode_deflate()
        .map_err(|error| format!("{error:?}"))?;
    let into = output
        .get_mut(..decoded.len())
        .ok_or("decoded past its output")?;
    into.copy_from_slice(&decoded);
    Ok(decoded.len())
}

/// Where a decoder that writes what it decodes puts it: `output`, taken
/// in pieces of at most `piece` bytes.
struct Room<'a> {
    output: &'a mut [u8],
    piece: usize,
}

impl std::io::Write for Room<'_> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let len = buf.len().min(self.piece).min(self.output.len());
        if len == 0 && !buf.is_empty() {
            return Err(std::io::Error::other("decoded past its output"));
        }
        let output = std::mem::take(&mut self.output);
        let (into, rest) = output.split_at_mut(len);
        into.copy_from_slice(&buf[..len]);
        self.output = rest;
        Ok(len)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn yazi(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let failed = |error: yazi::Error| format!("{error:?}");
    let mut decoder = yazi::Decoder::new();
    if piece == usize::MAX {
        let mut stream = decoder.stream_into_buf(output);
        stream.write(input).map_err(failed)?;
        return Ok(stream.finish().map_err(failed)?.0 as usize);
    }
    let mut room = Room { output, piece };
    let mut stream = decoder.stream(&mut room);
    for given in input.chunks(piece) {
        stream.write(given).map_err(failed)?;
    }
    Ok(stream.finish().map_err(failed)?.0 as usize)
}

fn liblzma_lzma(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let stream = liblzma::stream::Stream::new_lzma_decoder(u64::MAX);
    liblzma(stream, input, output, piece)
}

fn liblzma_xz(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let stream = liblzma::stream::Stream::new_stream_decoder(u64::MAX, 0);
    liblzma(stream, input, output, piece)
}

/// Drives liblzma's decoder `stream`, whole in one call that may finish
/// it, as it decodes fastest, or in pieces.
fn liblzma(
    stream: Result<liblzma::stream::Stream, liblzma::stream::Error>,
    input: &[u8],
    output: &mut [u8],
    piece: usize,
) -> Result<usize, Failure> {
    use liblzma::stream::{Action, Status};
    let mut stream = stream.map_err(|error| error.to_string())?;
    let action = match piece {
        usize::MAX => Action::Finish,
        _ => Action::Run,
    };
    loop {
        let (read, written) = (stream.total_in() as usize, stream.total_out() as usize);
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let status = stream
            .process(&input[read..into], &mut output[written..room], action)
            .map_err(|error| error.to_string())?;
        if let Status::StreamEnd = status {
            return Ok(stream.total_out() as usize);
        }
        if (stream.total_in() as usize, stream.total_out() as usize) == (read, written) {
            return Err("cut short".into());
        }
    }
}

fn lzma_rust2_lzma(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut stream = lzma_rust2::LzmaStream::new_mem_limit(u32::MAX, None);
    lzma_rust2(|i, o, a| stream.process(i, o, a), input, output, piece)
}

fn lzma_rust2_xz(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut stream = lzma_rust2::XzStream::new(false);
    lzma_rust2(|i, o, a| stream.process(i, o, a), input, output, piece)
}

/// Drives one of `lzma-rust2`'s sans-I/O decoders, `process`, over
/// `input` into `output`, in pieces of at most `piece` bytes each way,
/// saying so once it has all the input.
fn lzma_rust2<E, P>(
    mut process: P,
    input: &[u8],
    output: &mut [u8],
    piece: usize,
) -> Result<usize, Failure>
where
    E: std::fmt::Debug,
    P: FnMut(&[u8], &mut [u8], lzma_rust2::Action) -> Result<lzma_rust2::StreamResult, E>,
{
    use lzma_rust2::{Action, Status};
    let (mut read, mut written) = (0_usize, 0_usize);
    loop {
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let action = match into == input.len() {
            true => Action::Finish,
            false => Action::Run,
        };
        let result = process(&input[read..into], &mut output[written..room], action)
            .map_err(|error| format!("{error:?}"))?;
        let (consumed, produced) = (result.bytes_consumed, result.bytes_produced);
        (read, written) = (read + consumed, written + produced);
        if let Status::StreamEnd = result.status {
            return Ok(written);
        }
        if (consumed, produced) == (0, 0) {
            return Err("cut short".into());
        }
    }
}

/// Whole, from the input into the output, as `Read` and `Write`; in
/// pieces, by its streaming decoder, written to in pieces.
fn lzma_rs_lzma(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use std::io::Write as _;
    let len = output.len();
    if piece == usize::MAX {
        let mut room = output;
        lzma_rs::lzma_decompress(&mut &input[..], &mut room).map_err(|e| e.to_string())?;
        return Ok(len - room.len());
    }
    let mut stream = lzma_rs::decompress::Stream::new(Room { output, piece });
    for given in input.chunks(piece) {
        stream.write_all(given).map_err(|error| error.to_string())?;
    }
    let room = stream.finish().map_err(|error| error.to_string())?;
    Ok(len - room.output.len())
}

/// Whole, from the input into the output, as `Read` and `Write`; in
/// pieces, read and written in pieces, as it has no streaming decoder of
/// xz.
fn lzma_rs_xz(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let len = output.len();
    if piece == usize::MAX {
        let mut room = output;
        lzma_rs::xz_decompress(&mut &input[..], &mut room).map_err(|e| e.to_string())?;
        return Ok(len - room.len());
    }
    let source = Pieces {
        bytes: input,
        piece,
    };
    let mut source = std::io::BufReader::with_capacity(piece, source);
    let mut room = Room { output, piece };
    lzma_rs::xz_decompress(&mut source, &mut room).map_err(|error| error.to_string())?;
    Ok(len - room.output.len())
}

/// Its dictionary grows as the stream asks, as ours does, rather than
/// taking 8 MiB from the start.
fn xz4rust(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use xz4rust::{DICT_SIZE_MAX, DICT_SIZE_MIN, XzDecoder, XzNextBlockResult};
    let mut decoder = XzDecoder::in_heap_with_alloc_dict_size(DICT_SIZE_MIN, DICT_SIZE_MAX);
    let (mut read, mut written) = (0_usize, 0_usize);
    loop {
        let into = input.len().min(read.saturating_add(piece));
        let room = output.len().min(written.saturating_add(piece));
        let result = decoder
            .decode(&input[read..into], &mut output[written..room])
            .map_err(|error| format!("{error:?}"))?;
        let (consumed, produced, ended) = match result {
            XzNextBlockResult::NeedMoreData(consumed, produced) => (consumed, produced, false),
            XzNextBlockResult::EndOfStream(consumed, produced) => (consumed, produced, true),
        };
        (read, written) = (read + consumed, written + produced);
        if ended {
            return Ok(written);
        }
        if (consumed, produced) == (0, 0) {
            return Err("cut short".into());
        }
    }
}

/// Drives a `bzip2` crate's `Decompress`, of either version, as libbzip2's
/// own interface is driven: a stream, then what follows it, unread.
macro_rules! bzip2_decompress {
    ($name:ident, $krate:ident) => {
        fn $name(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
            let mut decoder = $krate::Decompress::new(false);
            loop {
                let (read, written) = (decoder.total_in() as usize, decoder.total_out() as usize);
                let into = input.len().min(read.saturating_add(piece));
                let room = output.len().min(written.saturating_add(piece));
                let status = decoder
                    .decompress(&input[read..into], &mut output[written..room])
                    .map_err(|error| error.to_string())?;
                if status == $krate::Status::StreamEnd {
                    return Ok(decoder.total_out() as usize);
                }
                let stuck = decoder.total_in() as usize == read;
                if stuck && decoder.total_out() as usize == written {
                    return Err("cut short".into());
                }
            }
        }
    };
}

bzip2_decompress!(libbzip2, bzip2_c);
bzip2_decompress!(libbz2_rs, libbz2_rs);

/// A reader that gives `bytes` in pieces of at most `piece` bytes, as a
/// file or a socket might.
struct Pieces<'a> {
    bytes: &'a [u8],
    piece: usize,
}

impl Read for Pieces<'_> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let len = buf.len().min(self.piece).min(self.bytes.len());
        buf[..len].copy_from_slice(&self.bytes[..len]);
        self.bytes = &self.bytes[len..];
        Ok(len)
    }
}

/// Reads all `reader` gives into `output`, in pieces of at most `piece`
/// bytes.
fn read_into(mut reader: impl Read, output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    let mut written = 0_usize;
    loop {
        let room = output.len().min(written.saturating_add(piece));
        match reader.read(&mut output[written..room]) {
            Ok(0) if written < output.len() => return Ok(written),
            Ok(0) => {
                // Full: anything more is more than the stream holds.
                let mut more = [0];
                return match reader.read(&mut more) {
                    Ok(0) => Ok(written),
                    Ok(_) => Err("decoded past its output".into()),
                    Err(error) => Err(error.to_string()),
                };
            },
            Ok(len) => written += len,
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn bzip2_rs(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use bzip2_rs::decoder::{Decoder, ReadState, WriteState};
    let (mut decoder, mut rest, mut written) = (Decoder::new(), input, 0_usize);
    // It fails if asked to read into no room: once the output is full,
    // it reads into a byte of its own, which must stay empty. Version
    // 0.1.2 also refuses, as a truncated Huffman bitstream, streams of
    // random bytes that libbzip2 encodes and decodes, from about 100 KB,
    // and some others at level 1, such as Silesia's `mozilla`: so it is
    // disqualified on them.
    let mut past = [0];
    loop {
        let room = output.len().min(written.saturating_add(piece));
        let into = match room > written {
            true => &mut output[written..room],
            false => &mut past[..],
        };
        match decoder.read(into) {
            Ok(ReadState::NeedsWrite(_)) => {
                // An empty write says the stream has ended.
                let given = &rest[..rest.len().min(piece)];
                match decoder.write(given) {
                    Ok(WriteState::Written(len)) => rest = &rest[len..],
                    Ok(WriteState::NeedsRead) => return Err("refused a write".into()),
                    Err(error) => return Err(error.to_string()),
                }
            },
            Ok(ReadState::Read(len)) if room > written => written += len,
            Ok(ReadState::Read(_)) => return Err("decoded past its output".into()),
            Ok(ReadState::Eof) => return Ok(written),
            Err(error) => return Err(error.to_string()),
        }
    }
}

fn libzstd(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use zstd::stream::raw::{InBuffer, Operation, OutBuffer};
    if piece == usize::MAX {
        return zstd::bulk::decompress_to_buffer(input, output).map_err(|error| error.to_string());
    }
    let mut decoder = zstd::stream::raw::Decoder::new().map_err(|error| error.to_string())?;
    let (mut read, mut written) = (0, 0);
    loop {
        let into = input.len().min(read + piece);
        let room = output.len().min(written + piece);
        let mut source = InBuffer::around(&input[read..into]);
        let mut sink = OutBuffer::around(&mut output[written..room]);
        let hint = decoder
            .run(&mut source, &mut sink)
            .map_err(|error| error.to_string())?;
        let (consumed, produced) = (source.pos(), sink.pos());
        (read, written) = (read + consumed, written + produced);
        // A frame ended, and the input with it.
        if hint == 0 && read == input.len() {
            return Ok(written);
        }
        if (consumed, produced) == (0, 0) {
            return Err("cut short".into());
        }
    }
}

/// The largest window zstd decodes by default, and so `--long=27`.
const WINDOW: u64 = 128 << 20;

fn ruzstd(input: &[u8], output: &mut [u8], piece: usize) -> Result<usize, Failure> {
    use ruzstd::decoding::{FrameDecoder, StreamingDecoder};
    // Its default is 100 MiB.
    let mut decoder = FrameDecoder::new();
    decoder.set_max_window_size(WINDOW);
    if piece == usize::MAX {
        return decoder
            .decode_all(input, output)
            .map_err(|error| error.to_string());
    }
    let source = Pieces {
        bytes: input,
        piece,
    };
    let decoder = StreamingDecoder::new_with_decoder(source, decoder).map_err(|e| e.to_string())?;
    read_into(decoder, output, piece)
}
