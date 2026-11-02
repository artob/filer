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

`lib/filer-ntfs/tests/external.sh` tests `filer-ntfs` against these NTFS
disk images. They would only bloat this repository, so none is in it:
the script fetches them at the commit it names, checks each by its
SHA-256 sum in `lib/filer-ntfs/tests/external.sha256`, and unpacks each
disk's NTFS partition.

- [ntfs-samples](https://github.com/msuhanov/ntfs-samples), by Maxim
  Suhanov, dedicated to the public domain under CC0 1.0 Universal, but
  for the boot code in the images: its six images, of Windows 10
  volumes with large clusters, slack, compression, diverging times, and
  an index entry left in MFT slack.
