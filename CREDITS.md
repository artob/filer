# Credits

## Test fixtures

`lib/filer-xfs/tests/external.sh` tests `filer-xfs` against the XFS
volumes of these projects' test suites. They are not public domain, so
none is in this repository: the script fetches just their fixture files,
at the commits it names, and checks each by its SHA-256 sum in
`lib/filer-xfs/tests/external.sha256`. None of their code was read.

- [go-xfs-filesystem](https://github.com/masahiro331/go-xfs-filesystem),
  by masahiro331, under the Apache License 2.0: `xfs/testdata`.
- [xfuse](https://github.com/KhaledEmaraDev/xfuse), by Khaled Emara,
  under the BSD 2-Clause License: `resources`.
- [lamxfs](https://github.com/lamco-admin/lamxfs), by Lamco Development
  LLC, under the MIT License or the Apache License 2.0:
  `tests/fixtures`, its images and the trees recorded of them.
