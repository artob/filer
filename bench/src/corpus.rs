// This is free and unencumbered software released into the public domain.

//! The corpus: the sources `corpus.toml` names, fetched, verified, and
//! unpacked into inputs, beside the synthetic ones; and what the reference
//! encoders make of every input, encoded once and recorded in
//! `packed.toml`, so that every machine decodes the same bytes.

use crate::{
    decoders::Codec,
    inputs::{self, Level},
};
use serde::Deserialize;
use std::{
    collections::BTreeMap,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

#[derive(Deserialize)]
struct Manifest {
    source: Vec<Source>,
}

/// A file fetched from `url`, and the inputs taken from it.
#[derive(Deserialize)]
struct Source {
    name: String,
    url: String,
    sha256: String,
    inputs: Vec<Entry>,
}

/// An input: the file at `path` in its source, a ZIP file or a tarball,
/// or else its source decompressed whole, cut to `bytes` if given.
#[derive(Deserialize)]
struct Entry {
    name: String,
    path: Option<String>,
    bytes: Option<usize>,
    sha256: String,
}

/// The directory of `corpus.toml`, `packed.toml`, and the corpus itself.
fn bench() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn corpus() -> PathBuf {
    bench().join("corpus")
}

fn manifest() -> Result<Manifest> {
    Ok(toml::from_str(&fs::read_to_string(
        bench().join("corpus.toml"),
    )?)?)
}

pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().fold(String::new(), |mut hex, byte| {
        let _ = write!(hex, "{byte:02x}");
        hex
    })
}

fn sha256(bytes: &[u8]) -> String {
    hex(&compute_sha256::sha256(bytes))
}

/// Whether `path` holds bytes whose SHA-256 is `sha256`.
fn verified(path: &Path, sha256: &str) -> bool {
    fs::read(path).is_ok_and(|bytes| self::sha256(&bytes) == sha256)
}

/// Fetches every source that is not yet here and verified, and unpacks
/// and verifies every input of it that is not.
pub fn fetch() -> Result<()> {
    for source in manifest()?.source {
        let file = source.url.rsplit('/').next().ok_or("no file name")?;
        let path = corpus().join("sources").join(file);
        if !verified(&path, &source.sha256) {
            eprintln!("fetching {}", source.url);
            fs::create_dir_all(corpus().join("sources"))?;
            let status = Command::new("curl")
                .args(["-sSfL", "-o"])
                .arg(&path)
                .arg(&source.url)
                .status()?;
            if !status.success() || !verified(&path, &source.sha256) {
                return Err(format!("{}: not as recorded", source.name).into());
            }
        }
        for entry in &source.inputs {
            let input = corpus().join("inputs").join(&entry.name);
            if verified(&input, &entry.sha256) {
                continue;
            }
            eprintln!("unpacking {}", entry.name);
            let bytes = unpack(&path, entry)?;
            if sha256(&bytes) != entry.sha256 {
                return Err(format!("{}: not as recorded", entry.name).into());
            }
            fs::create_dir_all(input.parent().ok_or("no directory")?)?;
            fs::write(&input, bytes)?;
        }
    }
    Ok(())
}

/// Returns `entry`'s bytes from `source`: by `unzip` or `tar`, which
/// find compression themselves, or decompressed whole by `decode-xz`.
fn unpack(source: &Path, entry: &Entry) -> Result<Vec<u8>> {
    let output = |command: &mut Command| -> Result<Vec<u8>> {
        let output = command.output()?;
        match output.status.success() {
            true => Ok(output.stdout),
            false => Err(String::from_utf8_lossy(&output.stderr).into()),
        }
    };
    let zip = source
        .extension()
        .is_some_and(|extension| extension == "zip");
    let mut bytes = match &entry.path {
        Some(path) if zip => output(Command::new("unzip").arg("-p").arg(source).arg(path))?,
        Some(path) => output(Command::new("tar").arg("-xOf").arg(source).arg(path))?,
        None => unxz(&fs::read(source)?, entry.bytes.unwrap_or(usize::MAX))?,
    };
    bytes.truncate(entry.bytes.unwrap_or(usize::MAX));
    Ok(bytes)
}

/// Decodes the xz stream `packed`, to at most `most` bytes.
fn unxz(mut packed: &[u8], most: usize) -> Result<Vec<u8>> {
    let mut decoder = decode_xz::Decoder::new();
    let (mut output, mut buf) = (Vec::new(), vec![0; 1 << 20]);
    while output.len() < most {
        let (consumed, produced) = decoder
            .decode(packed, &mut buf)
            .map_err(|error| error.to_string())?;
        packed = &packed[consumed..];
        output.extend_from_slice(&buf[..produced]);
        if (consumed, produced) == (0, 0) {
            decoder.finish().map_err(|error| error.to_string())?;
            break;
        }
    }
    Ok(output)
}

/// An input, by name, and where its bytes are, or that it is synthetic.
pub struct Input {
    pub name: String,
    synthetic: Option<Vec<u8>>,
}

impl Input {
    pub fn bytes(&self) -> Result<Vec<u8>> {
        match &self.synthetic {
            Some(bytes) => Ok(bytes.clone()),
            None => Ok(fs::read(corpus().join("inputs").join(&self.name))?),
        }
    }

    /// Where its encoding by `codec` at `level` is.
    fn packed(&self, codec: Codec, level: Level) -> PathBuf {
        let name = format!("{}.{}", self.name, codec.extension());
        let directory = format!("{}-{}", codec.name(), level.label());
        corpus().join("packed").join(directory).join(name)
    }
}

/// The inputs whose names begin with `prefix`: the synthetic ones, and
/// the corpus's, which must have been fetched.
pub fn inputs(prefix: &str) -> Result<Vec<Input>> {
    let synthetic = inputs::synthetic().into_iter().map(|input| Input {
        name: format!("synthetic/{}", input.name),
        synthetic: Some(input.data),
    });
    let manifest = manifest()?;
    let corpus = manifest.source.into_iter().flat_map(|source| source.inputs);
    let corpus = corpus.map(|entry| Input {
        name: entry.name,
        synthetic: None,
    });
    let all = synthetic.chain(corpus);
    Ok(all.filter(|input| input.name.starts_with(prefix)).collect())
}

/// `packed.toml`: the SHA-256 of each input's encoding, by codec and
/// level, and then by input's name.
type Packed = BTreeMap<String, BTreeMap<String, String>>;

fn packed() -> Result<Packed> {
    match fs::read_to_string(bench().join("packed.toml")) {
        Ok(text) => Ok(toml::from_str(&text)?),
        Err(_) => Ok(Packed::new()),
    }
}

/// Encodes each input of `inputs` with `codec` at each of its levels,
/// unless already encoded as `packed.toml` records, and checks each
/// encoding against `packed.toml`, recording it there if new: the
/// reference encoders are deterministic, so that every machine may encode
/// the same bytes.
pub fn encode(codec: Codec, inputs: &[Input]) -> Result<()> {
    let mut packed = packed()?;
    for input in inputs {
        let mut data = None;
        for level in codec.levels(input.size()?) {
            let path = input.packed(codec, level);
            let key = format!("{}-{}", codec.name(), level.label());
            let sha256 = packed.get(&key).and_then(|inputs| inputs.get(&input.name));
            let sha256 = sha256.cloned();
            if sha256
                .as_ref()
                .is_some_and(|sha256| verified(&path, sha256))
            {
                continue;
            }
            eprintln!("encoding {} with {} at {level}", input.name, codec.name());
            let data = match &mut data {
                Some(data) => data,
                None => data.insert(input.bytes()?),
            };
            let bytes = codec.encode(data, level)?;
            let hash = self::sha256(&bytes);
            if sha256.is_some_and(|sha256| sha256 != hash) {
                return Err(format!("{}: encoded otherwise than recorded", input.name).into());
            }
            fs::create_dir_all(path.parent().ok_or("no directory")?)?;
            fs::write(&path, bytes)?;
            packed
                .entry(key)
                .or_default()
                .insert(input.name.clone(), hash);
            // Recorded as it goes, as encoding all takes long.
            write_packed(&packed)?;
        }
    }
    Ok(())
}

fn write_packed(packed: &Packed) -> Result<()> {
    let mut text = String::from(
        "# Generated by `filer-bench encode`: the SHA-256 of each input as the\n\
         # reference encoders encode it, by codec and level.\n",
    );
    for (key, inputs) in packed {
        writeln!(text, "\n[{key}]")?;
        for (name, sha256) in inputs {
            writeln!(text, "{name:?} = {sha256:?}")?;
        }
    }
    Ok(fs::write(bench().join("packed.toml"), text)?)
}

impl Input {
    /// Its size, without reading it.
    pub fn size(&self) -> Result<usize> {
        match &self.synthetic {
            Some(bytes) => Ok(bytes.len()),
            None => Ok(fs::metadata(corpus().join("inputs").join(&self.name))?.len() as usize),
        }
    }

    /// Its encoding by `codec` at `level`, checked against `packed.toml`.
    pub fn encoded(&self, codec: Codec, level: Level) -> Result<Vec<u8>> {
        let bytes = fs::read(self.packed(codec, level))?;
        let key = format!("{}-{}", codec.name(), level.label());
        let packed = packed()?;
        let sha256 = packed.get(&key).and_then(|inputs| inputs.get(&self.name));
        match sha256 == Some(&self::sha256(&bytes)) {
            true => Ok(bytes),
            false => Err(format!("{}: not as recorded at {level}", self.name).into()),
        }
    }
}
