# Release checks

Releases still require maintainer approval. Bump the workspace version in a focused PR,
verify the exact commit's remote checks, and merge it into `main` before pushing its matching
`v<version>` tag. Do not move a published tag or rebuild a different commit under its name.

The tag workflow validates the Cargo version and checks that the tagged commit belongs to
`main`. It then calls the same CI, KCC comparison and [EPUB conformance](../tools/epubcheck/README.md)
workflows used for PRs, on that exact commit, with the extended pre-release parity cases
enabled, before building the four existing release targets. A failing validation, test,
advisory check, notice generation, comparison, EPUB check or build blocks publication.

Only the publication job has repository write permission. It downloads the `mangapress-*`
package artifacts, not the separate CI license reports, generates checksums, and requires
the release tag to exist. Archive names and installer entry points are unchanged.

## 0.8.2

This release gives a PDF bookmarks. It adds and removes no CLI flags or output formats, and JSON
protocol version 1 is unchanged. The pages of every format, and the title and author in a PDF, are
the same as before.

Release highlights:

- A PDF has a bookmark for each chapter, opening the chapter's first page, named as the EPUB's
  table of contents names it (the folder of the chapter; the book's title for pages lying directly in
  the book). A reader lists them beside the pages, and a long book has a way to reach a chapter
  other than turning pages. Titles are kept in any language. KCC's PDF has no bookmarks, so this is
  a difference from it, recorded with its reason in
  [ADR 0026](adr/0026-a-pdf-has-bookmarks.md).
- With `--nested-toc`, a PDF's bookmarks have two levels: one for each volume, closed, with its
  chapters under it, as an EPUB's table of contents has them. This is for the archive Mangabind
  writes for a whole series (`mangabind -combine`). Read back with KOReader's MuPDF, a series of
  three volumes lists each volume with its chapters under it, each on the page it opens.
- `--nested-toc` is accepted for a PDF, where it was refused. It is still refused for a CBZ, which
  has neither a table of contents nor bookmarks to nest: the error code is the same,
  `nested_toc_unsupported_format`, and its message now says `--format epub or --format pdf`.
  `--format auto` with `--nested-toc` is still an EPUB on every profile.
- The bookmarks of `ComicInfo.xml`, which an EPUB uses as its chapters when there are any, are not
  used for a PDF; its chapters come from the folders.

The EPUB and the CBZ are not changed. The code that names a chapter and groups chapters by volume
is now shared between the EPUB's navigation and the PDF's bookmarks (the EPUB written is
byte-identical to 0.8.1's), and the release notes of 0.8.1 already say that its PNG files are not
byte-identical to 0.8.0's.

The reference remains KCC 12.0.0. The tag workflow checks the exact merged release commit before
publication, including extended comparisons and complete-book EPUB conformance.

Mangabound's pinned toolchain will be updated separately; publishing this CLI release does not
change the desktop application until its pin moves.

## 0.8.1

This patch release fixes the page order of a CBZ made from a whole series, and updates `zip`
to clear three advisories. It adds and removes no CLI flags or output formats, and JSON
protocol version 1 is unchanged. The pixels of the pages written, and their order inside the
file, are the same as before; PNG pages are compressed by another library and their files differ
in size (see below).

Release highlights:

- Keep each chapter in the folders it was in when writing a CBZ. For input with a folder for each
  volume and a chapter folder inside it, such as the archive Mangabind writes for a whole series
  (`mangabind -combine`), the CBZ named each chapter by its last folder alone and added ` (2)` to
  repeats. A reader that orders pages by name, as KOReader's MuPDF does, showed the volumes
  interleaved: a twelve-page, two-volume test book opened in the order `6 7 8 0 1 2 9 10 11 3 4 5`
  and now opens `0 1 2 … 11`. Chapters keep their whole path (`v001 - Vol.01/c001/…`), as KCC 12.0.0
  keeps it, and two new comparisons with it fail on the old naming. The folder names inside a CBZ
  made from nested input change; input with one level of folders is written as before, and EPUB
  and PDF are unchanged. [ADR 0025](adr/0025-a-cbz-keeps-its-chapters-in-the-folders-they-were-in.md)
  records it.
- Update `zip` to 9.0.5, which clears RUSTSEC-2026-0367, -0368 and -0369. They concern archive
  extraction functions this tool does not call, so there was no exposure; the audit check now
  passes again. A name inside an input archive that is valid UTF-8 is read as UTF-8 whether or
  not the archive marks it, where `zip` 2 read unmarked names as code page 437. The compression
  methods that can be read are unchanged. The dependency list changed (the license notices accept
  `bzip2-1.0.6` for `libbz2-rs-sys` and `MIT-0` for `ppmd-rust`).
- **PNG files are not byte-identical to 0.8.0's.** `zip` 9 turns on the `zlib-rs` backend of `flate2`,
  which is also the library that compresses PNG pages, in place of `miniz_oxide`. The decoded pixels
  are the same (checked on the same pages with the 0.8.0 and 0.8.1 binaries, and by the comparison
  with KCC 12.0.0, which looks at pixels), and the files differ in size: from 1.9% smaller to 0.2%
  larger in the guide's option examples, 0.8% to 1.0% smaller on three plain test pages. JPEG pages
  are unchanged. As in the 0.8.0 notes, files are not claimed to be byte-identical to KCC's.

The reference remains KCC 12.0.0. The tag workflow checks the exact merged release commit before
publication, including extended comparisons and complete-book EPUB conformance.

Mangabound's pinned toolchain will be updated separately; publishing this CLI release does not
change the desktop application until its pin moves.

## 0.8.0

This release contains the input, ordering, metadata, and encoding fixes merged since 0.7.3,
the behavior-preserving module refactor, and the KOReader EPUB wrapper correction.
It intentionally changes automatic Kindle output to CBZ. Explicit formats and JSON protocol
version 1 remain available unchanged; no CLI flags or supported output formats were removed.

Release highlights:

- Make `--format auto` (including omitting `--format`) select CBZ for every Kindle profile.
  reMarkable still selects PDF, and Kobo/`OTHER` select EPUB. Use `--format epub` to keep
  EPUB output, chapter navigation, and resolved book metadata. CBZ does not generate
  chapter navigation or metadata; `--keepcomicinfo` can retain source ComicInfo unchanged.
- With `--nested-toc` and automatic format selection, produce EPUB on every profile,
  including older Kindles and reMarkable. Explicit CBZ/PDF with `--nested-toc` is still refused.
- Remove extra line height below page images in EPUB. KOReader's engine tests show that this
  closes the baseline gap while preserving proportions. It does not override the reader's
  margins or status bar; other readers and physical devices were not tested for this correction.
- Read CBZs with a single enclosing folder as the book they contain, including their metadata.
  Process a folder's own pages before its chapter subfolders, and use the book title for
  navigation entries that refer to pages directly in the book.
- Treat unreadable ComicInfo metadata as a warning, and read UTF-16 metadata. Correct signed
  issue-number padding, filename ordering, and custom-size palette/JPEG defaults.
- Recover available image data from truncated PNG/GIF pages, retaining the existing refusal
  of files whose required headers cannot be read.
- Write JPEG pages and covers with optimized Huffman tables and 4:2:0 color sampling, and
  improve PNG compression. Comparisons cover pixels, codec structure, and file size; files
  are not claimed to be byte-identical to KCC's.
- Separate CLI conversion phases, EPUB construction, and pipeline stages into focused modules.
  Preserve public interfaces and the original tests, with additional CLI boundary tests.
  This is a maintainability change, not a promised processing-speed improvement.

[ADR 0020](adr/0020-follow-kcc-on-the-differences-left-open.md) and
[ADR 0021](adr/0021-write-jpeg-pages-as-libjpeg-does.md) record the compatibility and encoding
decisions; [ADR 0022](adr/0022-behavior-preserving-module-layout.md) records the refactor.
[ADR 0023](adr/0023-no-line-height-around-a-page-image.md) records the wrapper correction,
and [ADR 0024](adr/0024-auto-format-is-cbz.md) records automatic format selection.
The reference remains KCC 12.0.0. The tag workflow checks the exact merged release commit
before publication, including extended comparisons and complete-book EPUB conformance.

Mangabound's pinned toolchain will be updated separately; publishing this CLI release does not
change the desktop application, which passes an explicit format.

## 0.7.3 (2026-10-08)

This patch release contains the October audit fixes and the EPUB follow-ups since 0.7.2.
It adds no new CLI flags or output formats; the JSON protocol remains version 1.
The audit and pre-release PRs are tracked in [audit follow-ups](audit-follow-ups.md).

Release highlights:

- Preserve existing outputs, validate portable output names, and stage completed writes.
  Unix output permissions follow ordinary file creation and the user's `umask`.
- Bound individual input reads, check image dimensions before decoding, and keep folder
  links inside the selected input. These checks are not a whole-process memory limit.
- Match KCC 12.0.0's format-specific Kindle DX target and Scribe EPUB width cap.
- Correct centered EPUB spine properties and nested NCX ordering. BMP input still works
  with processing; EPUB with `--noprocessing` now rejects BMP with a recovery message.
- Improve page-failure diagnostics, terminal help and pipe handling. Installers verify
  versioned downloads against release checksums and retain included license notices.
- Expand selected KCC comparisons and add a pinned EPUBCheck gate for 19 complete synthetic
  books, content assertions and failure controls. This is not exhaustive reader coverage.

[v0.7.3](https://github.com/gustavommcv/mangapress/releases/tag/v0.7.3) was published from
commit `9ab67b858ba51221b6ac1f9074908bc61cb49289`. The tag workflow passed three-platform CI,
extended KCC comparisons and complete-book EPUB conformance before publishing all four packages
and `checksums.txt`. Downloads matched the published checksums and contained the executable
and license notices. The downloaded Windows CLI reported version 0.7.3 and protocol version 1.
The reader smoke test below covers its stated pre-release candidate.
Mangabound's pinned toolchain is updated separately; this release does not change it.

### Reader smoke test

Use the exact candidate commit in an isolated checkout, build the native CLI with
`cargo build --release --locked -p mangapress-cli`, and record its `--version`. Run the
[complete-book gate](../tools/epubcheck/README.md) to generate fresh books; retain its
printed run directory, logs and `summary.json`. Do not open the intentionally broken
`missing-resource.epub` as a positive case.

In KOReader, inspect the ordinary JPEG and centered PNG books, nested volumes, bookmarks
after RTL splitting, color PNG, mixed-codec passthrough, Unicode/overridden metadata,
external cover, and a large Scribe case. Check actual page rendering, first/next/last pages,
TOC labels and destinations, cover and metadata. Use `cases.py` for the expected page order
and targets; XML assertions alone do not demonstrate reader behavior.

Record the Linux distribution/architecture, KOReader version and build, window or screen
dimensions, relevant reader settings, observed results and screenshots. Preserve personal
books and settings. Classify failures separately from unsupported reader features and
blocked checks; ADR 0013 already records KOReader's handling of spine-placement properties.
Small fixtures do not assess manga quality, and full-size cases stretch synthetic patterns
only to exercise dimensions. If the test uses desktop KOReader, state that clearly: it
does not verify a physical Kindle/Kobo, e-ink refresh or device performance.

### Reader smoke test result (2026-10-08)

Candidate `5416bee8ef4b20938097bfc5d50af5693b826824` was tested on Arch Linux x86_64
with desktop KOReader 2026.07.1, an isolated reader profile and a 536 × 724 viewport
(half the Kindle 11 dimensions, with the same aspect ratio). The native CLI was built
with Rust 1.99.0, not the pinned 1.98.1; the repository's CI remains the authority for
the release compiler.

The gate passed all 19 positive books, both BMP passthrough refusals and the broken-resource
control. Reader checks passed for rendering, page order/counts, navigation targets, covers,
metadata, mixed codecs, color and large/custom dimensions. No reader finding blocked release.
Centered placement and page-side/direction properties were not observable in KOReader,
as already recorded in ADR 0013. Minor punctuation differences in its information screen
were not present in the validated OPF. This was not a physical-device, performance or
real-manga quality test; screenshots and logs were retained by the tester.

The test also found a Unix output-permission regression: staged files retained `tempfile`'s
0600 default after publication. Staging now requests ordinary file permissions through
the existing builder API, allowing the kernel to apply `umask` without changing it.
The temporary and published file share these permissions. CLI regression tests cover
EPUB, CBZ and PDF, explicit file and directory destinations, and ordinary/group-restricted/
owner-only masks. Windows staging and book content are unchanged. The release gates reran
CI, parity and EPUB conformance on the tagged commit before publication.

## Maintenance

- Rust 1.98.1 is the tested minimum and release compiler. The repository toolchain file is
  the source used by rustup in local development, CI, and release builds. Update it and the
  inherited Cargo `rust-version` together; rerun the three-platform checks and KCC comparison.
- External actions are pinned to commit SHAs with release labels. Review upstream changes
  before updating the pins. cargo-about 0.9.2, cargo-audit 0.22.2, and actionlint 1.7.12 are
  build/check tooling, not runtime dependencies.
- EPUBCheck 5.4.0's official ZIP is checksum-pinned in `tools/epubcheck/check.py`.
  Its Java runtime is test tooling only. Review version/digest updates together and run
  both the complete synthetic books and the intentionally broken-resource control.
- The RustSec database is intentionally current at each audit. Tool pins and a lockfile do
  not freeze security findings or GitHub-hosted runner images. Do not treat a database
  fetch failure as a clean audit.

Branch protection, Dependabot updates, and GitHub private vulnerability reporting are
repository settings or separate automation choices, not enabled by these workflow edits.
The [security policy](../SECURITY.md) provides the current private contact. Signing, SBOMs,
build provenance, new platforms, and scheduled parity monitoring remain outside this change.
