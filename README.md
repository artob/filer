# Filer

[![License](https://img.shields.io/badge/license-Public%20Domain-blue.svg)](https://unlicense.org)
[![Compatibility](https://img.shields.io/badge/rust-1.97%2B-blue)](https://endoflife.date/rust)

**Filer provides a filesystem API with platform-specific backends for Rust.**

**Status:** pre-release; no items are exported yet.

## 🛠️ Prerequisites

- [Rust] 1.97+

## 📚 Reference

### Crates

| Crate | Platforms | Summary |
| --- | --- | --- |
| `filer-api` | All | Platform-independent API |
| `filer-heap` | All | In-memory driver |
| `filer-posix` | Unix | POSIX backend |
| `filer-spec` | All | Executable driver specification, for tests |
| `filer-system` | All | Native backend for the target, plus the API |
| `filer-wasi` | WASI | WASI backend |
| `filer-whatwg` | Browsers | WHATWG File System (OPFS) backend |
| `filer-windows` | Windows | Windows backend |
| `filer-cli` | All | `filer` command-line interface |

## 👨‍💻 Development

```bash
git clone https://github.com/artob/filer.git
```

[Rust]: https://rust-lang.org
