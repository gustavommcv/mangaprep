# 31. Rename to mangaprep

Date: 2026-10-23

## Status

Accepted on 2026-10-23. The maintainer decided it, and approves it by merging the pull request that
carries it. The repository is renamed after that merge, by the maintainer's instruction.

## Context

The name mangapress is already used by other software and sites, and a search for it finds them, not
this project. The maintainer decided on 2026-10-23 to rename the tool to **mangaprep**. The
desktop app keeps its own name, Manga2Ink, and so does Mangabind. A name had been checked first: no
repository, user, npm, PyPI or crates.io package, and no search result for a product, is called
mangaprep (a search is not a trademark check).

The name was in more than words: the crates and the executable, the files of every release and the
installers' variables, the suffix of an output that would overwrite another, the temporary file a book
is staged in, the contributor line of every EPUB, the `tool` of the machine protocol, the address of
the repository.

## Decision

From 0.9.0 the tool is **mangaprep**:

- **Crates and executable:** `mangaprep-cli` and `mangaprep-core`, the executable `mangaprep`
  (`mangaprep.exe`).
- **Release files:** `mangaprep-<target>.tar.gz` or `.zip`, and `checksums.txt`. The files of earlier
  releases keep the names they had and stay where they are.
- **Installers:** `MANGAPREP_INSTALL_DIR`, `MANGAPREP_VERSION` and `MANGAPREP_NO_PATH_UPDATE`, and the
  folder of the licenses `mangaprep-licenses`. The old variables are not read: a script that sets them
  sets nothing, and the command it runs is another one.
- **What a run writes:** the suffix of an output that would overwrite another is ` (mangaprep)`, the
  staging file is `.mangaprep-*.tmp`, and the `dc:contributor` of an EPUB is `mangaprep-<version>`.
- **The machine protocol stays at version 1, and its `tool` is `"mangaprep"`.** The rule of
  `machine-protocol-v1.md` asks for a new version when a documented meaning changes. The meaning of
  `tool` (which tool wrote the stream) has not; its value has, with the name of the executable, in
  one release. A consumer that matches the value has to change anyway, since the command it runs
  is another, and the only consumer known (Manga2Ink) requires an exact release and checksum, which
  change with it. A version 2 would change nothing a consumer could act on. The alternative of
  keeping `"mangapress"` as a fixed identifier on the wire was left out: a stream that names a tool
  that no longer exists is what a reader of a log would be misled by.
- **The namespace of the books' identifiers does not change.** It is the version 5 UUID of the
  project's first address, `https://github.com/gustavommcv/mangapress`, and changing it would change
  the identifier of every book, those already made included. Its constant is `BOOK_NAMESPACE`, and
  the test that derives it from that address says so.
- **The repository is `github.com/gustavommcv/mangaprep`.** GitHub sends the old address to the
  new one, for the pages, the API, `git` and the files of releases, as long as no repository takes
  the old name.
- **The record keeps the old name.** Decision records, audits and the notes of releases up to 0.8.3
  say what was decided and released as mangapress and are left as written; the README says that the
  project was called mangapress, so that a person who knows it by that name finds it.

## Consequences

- A person or a script that runs `mangapress` has to run `mangaprep`; no executable with the old name
  is installed. An old installer address still works, since GitHub redirects the repository and
  the script at that address installs mangaprep.
- Books made by 0.9.0 name `mangaprep-0.9.0` as their contributor, and an output that would overwrite
  another has ` (mangaprep)`, where an earlier release wrote the old name.
- Manga2Ink moves its pin to the renamed tool in the same change that renames it in the app, since
  the executable, the files of the release and the `tool` of the protocol all change together.
- Behavior is not touched: the parity check passes against KCC 12.1.1 with the new name, as before.
