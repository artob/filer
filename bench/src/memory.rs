// This is free and unencumbered software released into the public domain.

//! What each decoder allocates, as `METHOD.md` asks: `memory` decodes every
//! input at every level once with each decoder, untimed, counting the heap
//! it allocates while it decodes, its output buffer not included: the most
//! it holds at once, and how many times it allocates or reallocates. The
//! count is deterministic, so one pass, in one process, suffices.
//!
//! On Linux with glibc, the harness replaces `malloc` and the functions
//! that go with it, delegating to glibc's own, so that the C counterparts'
//! allocations are counted as ours are, by the bytes `malloc` reserves for
//! each. Elsewhere, only what Rust allocates is counted, by the bytes it
//! asks for, and the C counterparts' figures are left out.

use crate::{Options, Result, corpus, decoders::Decoder, machine::Machine};
use serde::Serialize;
use std::{
    fs,
    io::Write as _,
    path::Path,
    sync::atomic::{AtomicBool, AtomicI64, AtomicU64, Ordering::Relaxed},
};

/// Whether allocations are counted now, the bytes allocated since counting
/// started, less those freed, the most of those, and the allocations.
static COUNTING: AtomicBool = AtomicBool::new(false);
static HELD: AtomicI64 = AtomicI64::new(0);
static PEAK: AtomicI64 = AtomicI64::new(0);
static ALLOCATIONS: AtomicU64 = AtomicU64::new(0);

/// Whether every allocation is counted, C's too.
const ALL: bool = cfg!(all(target_os = "linux", target_env = "gnu"));

/// Counts an allocation of `bytes`.
fn allocated(bytes: usize) {
    if COUNTING.load(Relaxed) {
        let bytes = bytes as i64;
        let held = HELD.fetch_add(bytes, Relaxed) + bytes;
        PEAK.fetch_max(held, Relaxed);
        ALLOCATIONS.fetch_add(1, Relaxed);
    }
}

/// Counts `bytes` freed.
fn freed(bytes: usize) {
    if COUNTING.load(Relaxed) {
        HELD.fetch_sub(bytes as i64, Relaxed);
    }
}

/// Runs `f`, and returns what it returns, the most bytes of heap it held at
/// once, and how many times it allocated.
fn counted<T>(f: impl FnOnce() -> T) -> (T, u64, u64) {
    HELD.store(0, Relaxed);
    PEAK.store(0, Relaxed);
    ALLOCATIONS.store(0, Relaxed);
    COUNTING.store(true, Relaxed);
    let value = f();
    COUNTING.store(false, Relaxed);
    let peak = PEAK.load(Relaxed).max(0) as u64;
    (value, peak, ALLOCATIONS.load(Relaxed))
}

/// glibc's `malloc` and its kin, replaced: each delegates to glibc's own,
/// which it exports as `__libc_*`, and counts the bytes `malloc` reserves.
/// `valloc` and `pvalloc`, which none of the decoders calls, are left as
/// they are.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod interposed {
    use super::{allocated, freed};
    use std::ffi::{c_int, c_void};

    unsafe extern "C" {
        fn __libc_malloc(size: usize) -> *mut c_void;
        fn __libc_calloc(count: usize, size: usize) -> *mut c_void;
        fn __libc_realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
        fn __libc_free(pointer: *mut c_void);
        fn __libc_memalign(align: usize, size: usize) -> *mut c_void;
        fn malloc_usable_size(pointer: *mut c_void) -> usize;
    }

    /// Counts `pointer` as allocated, if it is not null, and returns it.
    unsafe fn counted(pointer: *mut c_void) -> *mut c_void {
        if !pointer.is_null() {
            allocated(unsafe { malloc_usable_size(pointer) });
        }
        pointer
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn malloc(size: usize) -> *mut c_void {
        unsafe { counted(__libc_malloc(size)) }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut c_void {
        unsafe { counted(__libc_calloc(count, size)) }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void {
        let before = match pointer.is_null() {
            true => 0,
            false => unsafe { malloc_usable_size(pointer) },
        };
        let moved = unsafe { __libc_realloc(pointer, size) };
        // A null result with a size leaves the old block as it was.
        if !moved.is_null() || size == 0 {
            freed(before);
        }
        unsafe { counted(moved) }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn free(pointer: *mut c_void) {
        if !pointer.is_null() {
            freed(unsafe { malloc_usable_size(pointer) });
        }
        unsafe { __libc_free(pointer) }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn memalign(align: usize, size: usize) -> *mut c_void {
        unsafe { counted(__libc_memalign(align, size)) }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn aligned_alloc(align: usize, size: usize) -> *mut c_void {
        unsafe { counted(__libc_memalign(align, size)) }
    }

    #[unsafe(no_mangle)]
    unsafe extern "C" fn posix_memalign(out: *mut *mut c_void, align: usize, size: usize) -> c_int {
        const EINVAL: c_int = 22;
        const ENOMEM: c_int = 12;
        if !align.is_power_of_two() || align % size_of::<*mut c_void>() != 0 {
            return EINVAL;
        }
        let pointer = unsafe { counted(__libc_memalign(align, size)) };
        if pointer.is_null() {
            return ENOMEM;
        }
        unsafe { *out = pointer };
        0
    }
}

/// Rust's allocator, counted, where `malloc` is not replaced.
#[cfg(not(all(target_os = "linux", target_env = "gnu")))]
mod counting {
    use super::{allocated, freed};
    use std::alloc::{GlobalAlloc, Layout, System};

    struct Counting;

    #[global_allocator]
    static COUNTING: Counting = Counting;

    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            allocated(layout.size());
            unsafe { System.alloc(layout) }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            allocated(layout.size());
            unsafe { System.alloc_zeroed(layout) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, size: usize) -> *mut u8 {
            freed(layout.size());
            allocated(size);
            unsafe { System.realloc(pointer, layout, size) }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            freed(layout.size());
            unsafe { System.dealloc(pointer, layout) }
        }
    }
}

/// A line of the results' file, after the machine's.
#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
enum Record<'a> {
    Machine(&'a Machine),
    Memory {
        codec: &'a str,
        input: &'a str,
        level: String,
        piece: Option<usize>,
        decoder: &'a str,
        /// The size of the decoder's value, the most bytes of heap it held
        /// at once, and its allocations; the last two only if counted.
        value: usize,
        peak: Option<u64>,
        allocations: Option<u64>,
    },
    Failure {
        codec: &'a str,
        input: &'a str,
        level: String,
        decoder: &'a str,
        failure: &'a str,
    },
}

/// Whether `decoder`'s allocations are counted here.
fn counts(decoder: &Decoder) -> bool {
    ALL || decoder.language != "C"
}

/// The `memory` command: decodes every input at every level of each codec
/// once with each decoder, given and taken in pieces of `options.piece`,
/// counting what it allocates; prints a table of the most heap each held,
/// in KiB, and its allocations, and writes each to `bench/results/`.
pub fn run(options: &Options, bench: &Path) -> Result<()> {
    let machine = Machine::now(bench);
    fs::create_dir_all(bench.join("results"))?;
    let path = bench.join("results").join(machine.file_name());
    let mut results = fs::File::create(&path)?;
    let mut record = |record: &Record| -> Result<()> {
        writeln!(results, "{}", serde_json::to_string(record)?)?;
        Ok(())
    };
    record(&Record::Machine(&machine))?;
    eprintln!("writing {}", path.display());
    let inputs = corpus::inputs(&options.only)?;
    let piece = (options.piece != usize::MAX).then_some(options.piece);
    for &codec in &options.codecs {
        let decoders = codec.decoders();
        println!();
        println!("### {}", codec.name());
        println!();
        let sizes: Vec<String> = decoders
            .iter()
            .map(|decoder| format!("{} {} B", decoder.name, decoder.value))
            .collect();
        println!("The size of each decoder's value: {}.", sizes.join("; "));
        println!();
        print!("| Input | Level |");
        for decoder in decoders {
            print!(" {} |", decoder.name);
        }
        println!();
        print!("| --- | --- |");
        for _ in decoders {
            print!(" ---: |");
        }
        println!();
        for input in &inputs {
            let original = input.bytes()?;
            for level in codec.levels(original.len()) {
                let packed = input.encoded(codec, level)?;
                print!("| {} | {} |", input.name, level.label());
                for decoder in decoders {
                    let (codec, input, level) = (codec.name(), &input.name, level.label());
                    match decode(decoder, &packed, &original, options.piece) {
                        Err(failure) => {
                            print!(" ({failure}) |");
                            record(&Record::Failure {
                                codec,
                                input,
                                level,
                                decoder: decoder.name,
                                failure: &failure,
                            })?;
                        },
                        Ok((peak, allocations)) => {
                            let counted = counts(decoder);
                            match counted {
                                true => print!(" {:.1} KiB, {allocations} |", peak as f64 / 1024.0),
                                false => print!(" (not counted) |"),
                            }
                            record(&Record::Memory {
                                codec,
                                input,
                                level,
                                piece,
                                decoder: decoder.name,
                                value: decoder.value,
                                peak: counted.then_some(peak),
                                allocations: counted.then_some(allocations),
                            })?;
                        },
                    }
                }
                println!();
            }
        }
    }
    eprintln!("wrote {}", path.display());
    Ok(())
}

/// Decodes `packed` once with `decoder`, counting what it allocates, and
/// checks that it decoded `original`.
fn decode(
    decoder: &Decoder,
    packed: &[u8],
    original: &[u8],
    piece: usize,
) -> std::result::Result<(u64, u64), String> {
    let mut output = vec![0; original.len()];
    let (len, peak, allocations) = counted(|| (decoder.decode)(packed, &mut output, piece));
    match output[..len?] == *original {
        true => Ok((peak, allocations)),
        false => Err("wrong".into()),
    }
}
