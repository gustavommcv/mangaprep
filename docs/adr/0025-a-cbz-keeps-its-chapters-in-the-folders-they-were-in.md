# 25. A CBZ keeps its chapters in the folders they were in

Date: 2026-10-20

## Status

Proposed on 2026-10-20. The maintainer approves this record by merging the pull request that
carries it. It applies the row of [ADR 0013](0013-follow-a-named-kcc-release.md)'s table that says
chapters are keyed by their full path to the folders of the CBZ output, and adds no row.

## Context

Everywhere else a chapter is keyed by the whole path it had in the input ([ADR 0005](0005-mangabind-contract.md)).
The CBZ writer was the exception: it named a chapter's folder by the name of its last folder alone,
and put ` (2)`, ` (3)` on repeats, so that two chapters did not share a folder.

That is wrong for the archive Mangabind writes for a whole series (`mangabind -combine`), which has
one folder for each volume and one for each chapter in it, and numbers the chapters again from
`c001` in every volume:

```
v001 - Vol.01/c001/p0001.png   v001 - Vol.01/c002/p0001.png
v002 - Vol.02/c001/p0001.png   v002 - Vol.02/c002/p0001.png
```

Converted to a CBZ, this tool wrote `c001/`, `c002/`, `c001 (2)/`, `c002 (2)/`. A reader that orders
the pages by their names puts `c001 (2)/` before `c001/`, and the volumes alternate. KOReader is
such a reader: its MuPDF sorts the pages of a CBZ by the natural order of their whole names.
Measured with the MuPDF that KOReader 2026.07 bundles, on a book of twelve pages each of a
different gray level, the pages came out as `6 7 8 0 1 2 9 10 11 3 4 5`, where the archive that
Mangabind wrote itself is shown as `0 1 2 … 11`. The order inside the file was right; only the
names were not.

The repeated names are not the whole problem. With titled chapters (`c001 - A`, `c002 - B` in the
first volume, `c001 - C`, `c002 - D` in the second) nothing repeats and the volumes still
alternate, because the volume folder was dropped and the names that are left sort chapter number
first.

KCC 12.0.0 does not have the problem. Its `sanitizeTree` renames each folder within its parent
(adding an `A` only when two folders of the same parent would clash), numbers every page with one
running counter, and `makeZIP` zips the tree with its nesting kept, so what a reader sorts is what
the input held, in the input's order.

## Decision

- **A chapter's folder in a CBZ is the path of the folder it was in, relative to the input, one
  folder per level** (`v001 - Vol.01/c001 - One/p0001.png`). The pages are still named `p0001`,
  `p0002`, … within it.
- **Only real folder names are kept.** A level that would take a reader or an extractor out of the
  archive (`..`, a root, a drive) is dropped, and a `\` inside a name becomes `-`. Pages that lie
  directly in the book keep the title the chapter had (`Untitled`).
- **A folder name that two chapters would still share** (only possible if their paths differ just
  in what is dropped or replaced above) keeps the suffix, as before: the first has the plain name,
  the next `… (2)`.
- **EPUB and PDF are not changed.** Their chapter labels are still the title; a PDF has no folders.

## Alternatives considered

- **Number every chapter folder by its position (`0001 c001 - One`).** Any reader would sort it
  right, but the names stop being the input's, which is what a CBZ browsed by a person is meant to
  show (the source `.cbz` was), and KCC does not do it.
- **Add the volume folder only when a title repeats.** It fixes the repeated names and leaves the
  titled case above: whether the book comes out in order would depend on the names being unique.
- **Number the chapters of a series once, in Mangabind.** Mangabind's own CBZ is read right, and any
  other tool that writes a volume folder and a chapter folder has the same trouble here.

## Consequences

- A CBZ made from nested input has nested folders where it had one level. The pages are the same
  and in the same order; KOReader does not show folder names. Input with one level of folders, or
  pages directly in the book, gives the names it gave before.
- `tools/parity/books.py` has two new comparisons with KCC 12.0.0: a series whose volumes number
  their chapters again, with titled chapters and with untitled ones. They read each archive in the
  natural order of its names, as a reader does.
- The release notes of the first release that carries this say it.
