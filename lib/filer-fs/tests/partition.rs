// This is free and unencumbered software released into the public domain.

//! Partition tables as `sfdisk` makes them (`tests/fixtures/make.sh`),
//! read as `sfdisk -d` shows them, and tables no disk should have.

use filer_api::{BufResult, ErrorKind, ReadAt};
use filer_fs::partition::{self, Kind, PartitionError, Scheme, Table, Window};
use filer_spec::{Sparse, block_on};

/// What `sfdisk -d` shows of a partition: its number, first sector, count
/// of sectors, type, and, for GPT, GUID and name.
#[derive(Debug, PartialEq, Eq)]
struct Dumped {
    number: u32,
    start: u64,
    size: u64,
    kind: String,
    id: Option<String>,
    name: Option<String>,
}

/// Parses a dump of `sfdisk -d`, returning its sector size and partitions.
fn dumped(dump: &str) -> (u64, Vec<Dumped>) {
    let mut sector = 512;
    let mut partitions = Vec::new();
    for line in dump.lines() {
        if let Some(size) = line.strip_prefix("sector-size: ") {
            sector = size.parse().unwrap();
        }
        let Some((device, fields)) = line.split_once(" : ") else {
            continue;
        };
        // The device's trailing digits, as `gpt-4kp2`'s 2.
        let prefix = device.trim_end_matches(|c: char| c.is_ascii_digit());
        let number = device[prefix.len()..].parse().unwrap();
        let field = |key: &str| {
            fields.split(", ").find_map(|field| {
                field
                    .strip_prefix(key)
                    .and_then(|value| value.strip_prefix('='))
                    .map(str::trim)
            })
        };
        let name = field("name").map(|name| unescape(name.trim_matches('"')));
        partitions.push(Dumped {
            number,
            start: field("start").unwrap().parse().unwrap(),
            size: field("size").unwrap().parse().unwrap(),
            kind: field("type").unwrap().to_owned(),
            id: field("uuid").map(str::to_owned),
            name,
        });
    }
    (sector, partitions)
}

/// Decodes the `\xHH` escapes `sfdisk` writes non-ASCII bytes as.
fn unescape(text: &str) -> String {
    let mut bytes = Vec::new();
    let mut rest = text.as_bytes();
    while let Some((&byte, tail)) = rest.split_first() {
        if byte == b'\\'
            && let Some(hex) = tail.strip_prefix(b"x").and_then(|hex| hex.get(..2))
        {
            bytes.push(u8::from_str_radix(std::str::from_utf8(hex).unwrap(), 16).unwrap());
            rest = &tail[3..];
        } else {
            bytes.push(byte);
            rest = tail;
        }
    }
    String::from_utf8(bytes).unwrap()
}

/// Reads the table of fixture `name`.
fn table(name: &str) -> Table {
    let path = format!(
        "{}/tests/fixtures/{name}.sparse",
        env!("CARGO_MANIFEST_DIR")
    );
    let disk = Sparse::new(&std::fs::read(path).unwrap());
    block_on(partition::read(&disk)).unwrap().unwrap()
}

/// Checks that fixture `name` reads as its dump shows it.
fn agrees(name: &str, scheme: Scheme) {
    let dump = std::fs::read_to_string(format!(
        "{}/tests/fixtures/{name}.sfdisk",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap();
    let (sector, dumped) = dumped(&dump);
    let table = table(name);
    assert_eq!(
        (table.scheme(), table.sector_size()),
        (scheme, sector),
        "{name}"
    );
    let read: Vec<Dumped> = table
        .partitions()
        .iter()
        .map(|partition| {
            let (kind, id, name) = match partition.kind() {
                Kind::Mbr(kind) => (format!("{kind:x}"), None, None),
                Kind::Gpt { kind, id, name } => {
                    (kind.to_string(), Some(id.to_string()), Some(name.clone()))
                },
                kind => panic!("{kind:?}"),
            };
            Dumped {
                number: partition.number(),
                start: partition.start() / sector,
                size: partition.len() / sector,
                kind,
                id,
                name,
            }
        })
        .collect();
    // `sfdisk` lists an MBR's extended partition, which holds no volume.
    let dumped: Vec<Dumped> = dumped
        .into_iter()
        .filter(|partition| !matches!(partition.kind.as_str(), "5" | "f" | "85"))
        .collect();
    assert_eq!(read, dumped, "{name}");
}

#[test]
fn reads_mbr_tables_with_logical_partitions() {
    agrees("mbr", Scheme::Mbr);
    let numbers: Vec<u32> = table("mbr")
        .partitions()
        .iter()
        .map(|p| p.number())
        .collect();
    assert_eq!(numbers, [1, 2, 5, 6]);
}

#[test]
fn reads_gpts_of_each_sector_size() {
    agrees("gpt", Scheme::Gpt);
    agrees("gpt-4k", Scheme::Gpt);
}

#[test]
fn reads_gpts_by_their_backup_header() {
    agrees("gpt-backup", Scheme::Gpt);
}

/// An MBR sector naming `entries`, each a type, first sector, and count.
fn mbr(entries: &[(u8, u32, u32)]) -> Vec<u8> {
    let mut sector = vec![0; 512];
    for (i, &(kind, start, len)) in entries.iter().enumerate() {
        let at = 446 + 16 * i;
        sector[at + 4] = kind;
        sector[at + 8..at + 12].copy_from_slice(&start.to_le_bytes());
        sector[at + 12..at + 16].copy_from_slice(&len.to_le_bytes());
    }
    sector[510..512].copy_from_slice(&[0x55, 0xAA]);
    sector
}

fn error(disk: &[u8]) -> PartitionError {
    let error = block_on(partition::read(disk)).unwrap_err();
    assert_eq!(error.kind(), ErrorKind::Corrupt);
    *core::error::Error::source(&error)
        .unwrap()
        .downcast_ref::<PartitionError>()
        .unwrap()
}

#[test]
fn finds_no_table_where_there_is_none() {
    assert_eq!(block_on(partition::read(&[0u8; 4096][..])).unwrap(), None);
    assert_eq!(block_on(partition::read(&[0u8; 100][..])).unwrap(), None);
    // A FAT volume's boot sector has an MBR's signature.
    let mut fat = mbr(&[(0x0C, 2048, 100)]);
    fat[82..90].copy_from_slice(b"FAT32   ");
    fat.resize(1 << 20, 0);
    assert_eq!(block_on(partition::read(&fat)).unwrap(), None);
    // As do boot records with invalid boot flags.
    let mut flagged = mbr(&[(0x0C, 2048, 100)]);
    flagged[446] = 0x12;
    flagged.resize(1 << 20, 0);
    assert_eq!(block_on(partition::read(&flagged)).unwrap(), None);
}

#[test]
fn refuses_tables_no_disk_has() {
    let mut past = mbr(&[(0x83, 2048, 4096)]);
    past.resize(1 << 20, 0);
    assert_eq!(error(&past), PartitionError::OutOfRange { number: 1 });
    // An extended partition whose next boot record is itself.
    let mut looping = mbr(&[(0x05, 1, 2047)]);
    looping.resize(1 << 20, 0);
    looping[512..1024].copy_from_slice(&mbr(&[(0x83, 1, 10), (0x05, 0, 100)]));
    assert_eq!(error(&looping), PartitionError::BadExtended);
    // A protective MBR, and no GPT.
    let mut protective = mbr(&[(0xEE, 1, 2047)]);
    protective.resize(1 << 20, 0);
    assert_eq!(error(&protective), PartitionError::BadGpt);
}

#[test]
fn windows_read_their_range_only() {
    let disk: Vec<u8> = (0..=255).collect();
    let window = Window::new(disk, 100, 50);
    assert_eq!(block_on(window.size()).unwrap(), 50);
    let read = |off: u64, len: usize| {
        let BufResult(result, buf) = block_on(window.read_at(Vec::with_capacity(len), off));
        assert_eq!(result.unwrap(), buf.len());
        buf
    };
    assert_eq!(read(0, 3), [100, 101, 102]);
    assert_eq!(read(45, 100), [145, 146, 147, 148, 149]);
    assert_eq!(read(50, 10), Vec::<u8>::new());
    assert_eq!(read(u64::MAX, 10), Vec::<u8>::new());
}
