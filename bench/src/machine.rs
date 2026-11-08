// This is free and unencumbered software released into the public domain.

//! What a run records of where and how it ran: the machine, its state, the
//! build, the corpus, and the commit, as `METHOD.md` asks of raw data.

use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

/// What `tables` reads back, in which fields that older runs lack are
/// empty.
#[derive(Serialize, Deserialize, Default)]
#[serde(default)]
pub struct Machine {
    pub date: String,
    pub host: String,
    pub os: String,
    pub kernel: String,
    pub arch: String,
    pub cpu: String,
    pub cores: usize,
    pub memory: String,
    /// The hypervisor, if any, or `none`.
    pub virtualized: String,
    /// On Linux: the frequency governor, whether boost is on, and the CPUs
    /// the harness may run on; on macOS, none of these is set.
    pub governor: String,
    pub boost: String,
    pub cpus: String,
    pub rustc: String,
    pub rustflags: String,
    pub cc: String,
    pub cflags: String,
    pub packages: Vec<String>,
    /// The harness's features: `intrinsics`, by default, for our checksums
    /// with the CPU's instructions; none, for ours without dependencies.
    pub features: Vec<String>,
    /// The SHA-256 of `corpus.toml` and of `packed.toml`.
    pub corpus: String,
    pub packed: String,
    /// The commit, and whether a tracked file differs from it, but for the
    /// `BENCH.md` files that `tables` writes, which a run between others
    /// would otherwise count against them.
    pub commit: String,
    pub dirty: bool,
}

/// What `program` prints with `args`, trimmed, or nothing.
fn output(program: &str, args: &[&str]) -> String {
    Command::new(program)
        .args(args)
        .output()
        .ok()
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}

/// The contents of `path`, trimmed, or nothing.
fn read(path: &str) -> String {
    fs::read_to_string(path)
        .map(|text| text.trim().to_owned())
        .unwrap_or_default()
}

/// The value of the first line of `/proc/` file `path` that begins with
/// `key`, such as `/proc/cpuinfo`'s `model name`.
fn proc_field(path: &str, key: &str) -> String {
    let text = read(path);
    let line = text.lines().find(|line| line.starts_with(key));
    let value = line
        .and_then(|line| line.split_once(':'))
        .map(|(_, value)| value);
    value.unwrap_or_default().trim().to_owned()
}

impl Machine {
    pub fn now(bench: &Path) -> Self {
        let linux = cfg!(target_os = "linux");
        let sysctl = |key| output("sysctl", &["-n", key]);
        let hash = |name: &str| {
            let bytes = fs::read(bench.join(name)).unwrap_or_default();
            crate::corpus::hex(&compute_sha256::sha256(&bytes))
        };
        let cpu = match linux {
            true => proc_field("/proc/cpuinfo", "model name"),
            false => sysctl("machdep.cpu.brand_string"),
        };
        let memory = match linux {
            true => proc_field("/proc/meminfo", "MemTotal"),
            false => format!("{} bytes", sysctl("hw.memsize")),
        };
        let virtualized = match linux {
            true => output("systemd-detect-virt", &[]),
            false => match sysctl("kern.hv_vmm_present").as_str() {
                "1" => "yes".into(),
                _ => "none".into(),
            },
        };
        let cpufreq = "/sys/devices/system/cpu";
        Self {
            date: output("date", &["-u", "+%Y-%m-%dT%H:%M:%SZ"]),
            host: output("hostname", &[]),
            os: std::env::consts::OS.into(),
            kernel: output("uname", &["-sr"]),
            arch: std::env::consts::ARCH.into(),
            cpu,
            cores: std::thread::available_parallelism().map_or(0, usize::from),
            memory,
            virtualized,
            governor: read(&format!("{cpufreq}/cpu0/cpufreq/scaling_governor")),
            boost: read(&format!("{cpufreq}/cpufreq/boost")),
            cpus: proc_field("/proc/self/status", "Cpus_allowed_list"),
            rustc: env!("FILER_BENCH_RUSTC").into(),
            rustflags: env!("FILER_BENCH_RUSTFLAGS").into(),
            cc: env!("FILER_BENCH_CC").into(),
            cflags: env!("FILER_BENCH_CFLAGS").into(),
            packages: env!("FILER_BENCH_PACKAGES")
                .split(';')
                .map(Into::into)
                .collect(),
            features: match cfg!(feature = "intrinsics") {
                true => vec!["intrinsics".into()],
                false => Vec::new(),
            },
            corpus: hash("corpus.toml"),
            packed: hash("packed.toml"),
            commit: output("git", &["rev-parse", "HEAD"]),
            dirty: !output(
                "git",
                &[
                    "status",
                    "--porcelain",
                    "--untracked-files=no",
                    "--",
                    ":(top)",
                    ":(top,exclude,glob)lib/*/BENCH.md",
                ],
            )
            .is_empty(),
        }
    }

    /// Creates its results' file in `bench/results/`, named for the date
    /// and the host, `native` for a build for this CPU alone, and
    /// `intrinsics` for one with that; numbered if a run of the same second
    /// has one, as a short `memory` may.
    pub fn create(&self, bench: &Path) -> std::io::Result<(PathBuf, fs::File)> {
        let native = self.rustflags.contains("target-cpu=native");
        let mut build = String::from(if native { "-native" } else { "" });
        for feature in &self.features {
            build = format!("{build}-{feature}");
        }
        let name = format!("{}-{}{build}", self.date.replace(':', ""), self.host);
        let results = bench.join("results");
        fs::create_dir_all(&results)?;
        for n in 0.. {
            let path = match n {
                0 => results.join(format!("{name}.jsonl")),
                n => results.join(format!("{name}-{n}.jsonl")),
            };
            match fs::File::create_new(&path) {
                Ok(file) => return Ok((path, file)),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {},
                Err(error) => return Err(error),
            }
        }
        unreachable!("a results' file of each number exists")
    }
}
