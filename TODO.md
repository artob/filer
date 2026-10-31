# Enhancement backlog

Recheck the code before starting an item.
Paths below are relative to the repository root. Implementations live in
`lib/filer-{api,posix,cli}/src/`.
Sections are ordered by priority, and so are the items within each section.

## Working method

- Follow `AGENTS.md`. Implement one narrowly scoped, independently verifiable
  improvement per request. Split larger goals below into smaller steps before
  coding; an unchecked goal is not a request to implement its whole section.
- Each change should be suitable for one atomic commit. Create commits only
  when explicitly requested.
- Add meaningful regression coverage alongside behavior changes. After
  verification, remove fully completed items and retain only remaining substeps
  for partial work.
- Preserve `no_std`, `deny(unsafe_code)`, and feature gates. Prefer module/type
  rustdoc over expanding the README. The target MSRV is Rust 1.97.

## API

- [ ] **API-01: Define the initial `filer-api` surface.**
- [ ] **API-02: Implement it in `filer-posix`, gated on `cfg(unix)`.**
- [ ] **CLI-01: Give `filer-cli` argument parsing and a first command.**
