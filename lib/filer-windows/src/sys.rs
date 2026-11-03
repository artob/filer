// This is free and unencumbered software released into the public domain.

//! The system calls the driver makes that the standard library does not,
//! each a safe function over [`windows-sys`]. All of the crate's `unsafe`
//! is here (`DESIGN.md` §3.3).
//!
//! Handles are [`File`]s, which own and close them. Every function that
//! fails returns the Windows system error code, or for the native API the
//! `NTSTATUS`, which [`win32`] converts.
//!
//! [`windows-sys`]: https://docs.rs/windows-sys

use crate::{dir_info::Class, windows::SHARE_ALL};
use core::{ffi::c_void, mem::size_of, ptr};
use std::{
    fs::File,
    os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle},
    vec::Vec,
};
use windows_sys::{
    Wdk::{
        Foundation::OBJECT_ATTRIBUTES,
        Storage::FileSystem::{
            FILE_OPEN, FILE_SYNCHRONOUS_IO_NONALERT, FileIdBothDirectoryInformation,
            FileIdExtdDirectoryInformation, NtCreateFile, NtQueryDirectoryFile,
        },
    },
    Win32::{
        Foundation::{
            ERROR_INVALID_PARAMETER, GetLastError, NTSTATUS, OBJ_CASE_INSENSITIVE,
            RtlNtStatusToDosError, STATUS_INVALID_PARAMETER, STATUS_NO_MORE_FILES,
            STATUS_NO_SUCH_FILE, STATUS_OBJECT_NAME_INVALID, UNICODE_STRING,
        },
        Storage::FileSystem::{
            FILE_ATTRIBUTE_TAG_INFO, FILE_ID_128, FILE_ID_INFO, FILE_INFO_BY_HANDLE_CLASS,
            FileAttributeTagInfo, FileBasicInfo, FileIdInfo, FileStandardInfo,
            GetFileInformationByHandleEx, MAXIMUM_REPARSE_DATA_BUFFER_SIZE,
        },
        System::{
            IO::{DeviceIoControl, IO_STATUS_BLOCK},
            Ioctl::FSCTL_GET_REPARSE_POINT,
        },
    },
};

/// Returns the Windows system error code of the last failed call on this
/// thread.
fn last_error() -> u32 {
    // SAFETY: `GetLastError` only reads a thread-local value.
    unsafe { GetLastError() }
}

/// Converts an `NTSTATUS` into the Windows system error code it stands
/// for.
pub(crate) fn win32(status: NTSTATUS) -> u32 {
    // SAFETY: `RtlNtStatusToDosError` reads nothing but its argument (and
    // records it as the thread's last status).
    unsafe { RtlNtStatusToDosError(status) }
}

/// Opens `name`, a single component, relative to the directory `dir` (or
/// if `name` is empty, `dir` itself), with `access` and the create
/// `options`, as `NtCreateFile` does: with every share mode, for
/// synchronous I/O, matching names without regard to case as Win32 does
/// (how that meets per-directory case sensitivity is still VER-06).
///
/// The name is not parsed but by the file system, so a caller must refuse
/// names holding `\` (another component) or `:` (a stream).
pub(crate) fn open_relative(
    dir: &File,
    name: &[u16],
    access: u32,
    options: u32,
) -> Result<File, NTSTATUS> {
    let len = name
        .len()
        .checked_mul(2)
        .and_then(|len| u16::try_from(len).ok())
        .ok_or(STATUS_OBJECT_NAME_INVALID)?;
    let unicode = UNICODE_STRING {
        Length: len,
        MaximumLength: len,
        Buffer: name.as_ptr().cast_mut(),
    };
    let attributes = OBJECT_ATTRIBUTES {
        Length: size_of_u32::<OBJECT_ATTRIBUTES>(),
        RootDirectory: dir.as_raw_handle(),
        ObjectName: &raw const unicode,
        Attributes: OBJ_CASE_INSENSITIVE,
        SecurityDescriptor: ptr::null(),
        SecurityQualityOfService: ptr::null(),
    };
    let mut handle = ptr::null_mut();
    let mut io = IO_STATUS_BLOCK::default();
    // SAFETY: `NtCreateFile` only reads `attributes`, the name it points
    // to, and the root handle, all alive for the call; it writes the new
    // handle and the status block, both local. It retains none of them,
    // and the I/O is synchronous.
    let status = unsafe {
        NtCreateFile(
            &raw mut handle,
            access,
            &raw const attributes,
            &raw mut io,
            ptr::null(),
            0,
            SHARE_ALL,
            FILE_OPEN,
            options | FILE_SYNCHRONOUS_IO_NONALERT,
            ptr::null(),
            0,
        )
    };
    if status < 0 {
        return Err(status);
    }
    // SAFETY: the call succeeded, so `handle` is a new handle that nothing
    // else owns.
    Ok(File::from(unsafe { OwnedHandle::from_raw_handle(handle) }))
}

/// Opens the file `file` is open on again, with `access` and the create
/// `options`, as [`open_relative`] does: an empty name relative to a
/// handle names its file.
pub(crate) fn reopen(file: &File, access: u32, options: u32) -> Result<File, NTSTATUS> {
    open_relative(file, &[], access, options)
}

/// Fills `info`, the structure that `class` names, with information about
/// `file`.
///
/// # Safety
///
/// `class` must name a class whose structure is `T`, or one `T` begins
/// with, of no more than `T`'s size.
unsafe fn info<T>(file: &File, class: FILE_INFO_BY_HANDLE_CLASS, info: &mut T) -> Result<(), u32> {
    // SAFETY: `info` is writable for its size, and by the caller's promise
    // the class's structure fits it.
    let ok = unsafe {
        GetFileInformationByHandleEx(
            file.as_raw_handle(),
            class,
            ptr::from_mut(info).cast::<c_void>(),
            size_of_u32::<T>(),
        )
    };
    if ok == 0 { Err(last_error()) } else { Ok(()) }
}

/// Returns the attributes of `file` and, if it is a reparse point, its
/// tag.
pub(crate) fn attribute_tag(file: &File) -> Result<(u32, u32), u32> {
    let mut tag = FILE_ATTRIBUTE_TAG_INFO {
        FileAttributes: 0,
        ReparseTag: 0,
    };
    // SAFETY: `FileAttributeTagInfo` is `FILE_ATTRIBUTE_TAG_INFO`.
    unsafe { info(file, FileAttributeTagInfo, &mut tag)? };
    Ok((tag.FileAttributes, tag.ReparseTag))
}

/// A file's times, as NT ticks (0 where the file system keeps none), and
/// its attributes: `FILE_BASIC_INFO`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Basic {
    pub(crate) created: i64,
    pub(crate) accessed: i64,
    pub(crate) modified: i64,
    pub(crate) changed: i64,
    pub(crate) attributes: u32,
}

/// A file's sizes and links: `FILE_STANDARD_INFO`, with its two
/// `BOOLEAN`s as bytes, which any value is valid for.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Standard {
    pub(crate) allocation: i64,
    pub(crate) end_of_file: i64,
    pub(crate) links: u32,
    pub(crate) delete_pending: u8,
    pub(crate) directory: u8,
}

/// Returns the times and attributes of `file`.
pub(crate) fn basic(file: &File) -> Result<Basic, u32> {
    let mut basic = Basic::default();
    // SAFETY: `FileBasicInfo` is `FILE_BASIC_INFO`, laid out as `Basic`.
    unsafe { info(file, FileBasicInfo, &mut basic)? };
    Ok(basic)
}

/// Returns the sizes and link count of `file`.
pub(crate) fn standard(file: &File) -> Result<Standard, u32> {
    let mut standard = Standard::default();
    // SAFETY: `FileStandardInfo` is `FILE_STANDARD_INFO`, laid out as
    // `Standard` but for its `BOOLEAN`s, which are bytes in both.
    unsafe { info(file, FileStandardInfo, &mut standard)? };
    Ok(standard)
}

/// Returns the serial number of the volume `file` is on, and its 128-bit
/// id there.
pub(crate) fn id(file: &File) -> Result<(u64, u128), u32> {
    let mut id = FILE_ID_INFO {
        VolumeSerialNumber: 0,
        FileId: FILE_ID_128 {
            Identifier: [0; 16],
        },
    };
    // SAFETY: `FileIdInfo` is `FILE_ID_INFO`.
    unsafe { info(file, FileIdInfo, &mut id)? };
    Ok((
        id.VolumeSerialNumber,
        u128::from_le_bytes(id.FileId.Identifier),
    ))
}

/// Returns the reparse point `file` is open on (with
/// `FILE_OPEN_REPARSE_POINT`): its tag, data length, and data, as
/// [`filer_msft::reparse::Reparse::parse`] reads them.
pub(crate) fn reparse_point(file: &File) -> Result<Vec<u8>, u32> {
    // Words, for the alignment the structure has.
    let size =
        usize::try_from(MAXIMUM_REPARSE_DATA_BUFFER_SIZE).map_err(|_| ERROR_INVALID_PARAMETER)?;
    let mut words = std::vec![0u32; size.div_ceil(4)];
    let mut returned = 0;
    // SAFETY: the output buffer is writable for the size passed, and
    // `returned` is local; the I/O is synchronous, so nothing outlives the
    // call.
    let ok = unsafe {
        DeviceIoControl(
            file.as_raw_handle(),
            FSCTL_GET_REPARSE_POINT,
            ptr::null(),
            0,
            words.as_mut_ptr().cast::<c_void>(),
            MAXIMUM_REPARSE_DATA_BUFFER_SIZE,
            &raw mut returned,
            ptr::null_mut(),
        )
    };
    if ok == 0 {
        return Err(last_error());
    }
    let returned = usize::try_from(returned).map_err(|_| ERROR_INVALID_PARAMETER)?;
    Ok(words
        .iter()
        .flat_map(|word| word.to_ne_bytes())
        .take(returned)
        .collect())
}

/// Reads the next entries of the listing `dir` is open for, in `class`,
/// into `buf`, and returns their bytes, or none at the end.
pub(crate) fn query_directory(
    dir: &File,
    class: Class,
    buf: &mut [u64],
) -> Result<Vec<u8>, NTSTATUS> {
    let class = match class {
        Class::Extd => FileIdExtdDirectoryInformation,
        Class::Both => FileIdBothDirectoryInformation,
    };
    let len = buf
        .len()
        .checked_mul(8)
        .and_then(|len| u32::try_from(len).ok())
        .ok_or(STATUS_INVALID_PARAMETER)?;
    let mut io = IO_STATUS_BLOCK::default();
    // SAFETY: `buf` is writable for the length passed, and aligned for
    // every class's records; `io` is local. The handle is synchronous, so
    // the call is done with both when it returns.
    let status = unsafe {
        NtQueryDirectoryFile(
            dir.as_raw_handle(),
            ptr::null_mut(),
            None,
            ptr::null(),
            &raw mut io,
            buf.as_mut_ptr().cast::<c_void>(),
            len,
            class,
            false,
            ptr::null(),
            false,
        )
    };
    match status {
        STATUS_NO_MORE_FILES | STATUS_NO_SUCH_FILE => return Ok(Vec::new()),
        status if status < 0 => return Err(status),
        _ => {},
    }
    let returned = io.Information;
    Ok(buf
        .iter()
        .flat_map(|word| word.to_ne_bytes())
        .take(returned)
        .collect())
}

/// Returns the size of `T`, which for the structures passed to the system
/// is far below 4 GiB (and if not, 0, which the system refuses).
fn size_of_u32<T>() -> u32 {
    u32::try_from(size_of::<T>()).unwrap_or(0)
}
