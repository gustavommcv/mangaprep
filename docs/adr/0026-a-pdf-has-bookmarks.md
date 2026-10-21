# 26. A PDF has bookmarks

Date: 2026-10-21

## Status

Proposed on 2026-10-21. The maintainer approves this record by merging the pull request that
carries it. It adds a difference to the table of [ADR 0013](0013-follow-a-named-kcc-release.md)
and replaces the part of [ADR 0012](0012-nested-toc-for-combined-volumes.md) that kept
`--nested-toc` out of a PDF.

## Context

A PDF made by this tool had its pages and its title and author, and nothing to move between
chapters with: no bookmarks. KCC's own PDF is the same (`buildPDF` writes metadata and pages and
no outline), so this was not a difference from it, and not one anybody had asked about, until the
book was a whole series. A series made into one PDF is hundreds of pages with no way to reach a
volume or a chapter but turning pages (Mangabound ADR 0081 offers that book in every format).

ADR 0012 gave a combined series a two-level table of contents in an EPUB and refused `--nested-toc`
for a PDF, because the PDF library, `printpdf`, "hardcodes every bookmark's parent to the root". That
is what its `add_bookmark` does, and it also sorts the bookmarks by page and then by name, so the
bookmark of a volume and that of its first chapter, which open the same page, come out in the order
of their names, the chapter first. The library hands over the `lopdf` document it is about to save
(`PdfDocument::to_lopdf_document`), and an outline of any shape can be written into that.

A PDF reader lists the outline beside the pages. KOReader's MuPDF reads the nesting: checked on a
two-level outline of this tool, its table of contents shows each volume with its chapters under it,
each on the page it opens.

## Decision

- **A PDF has a bookmark for each chapter**, named as the EPUB's contents name it (the folder of
  the chapter; the book's title for pages lying directly in the book), opening the chapter's first
  page. The rule is shared with the EPUB's navigation (`ebook/contents.rs`), so the two agree.
- **With `--nested-toc` the outline has two levels**: a bookmark for each volume, opening where its
  first chapter does, with the chapters under it, as the EPUB's does. A volume starts closed.
- **`--nested-toc` is accepted for a PDF and refused for a CBZ only**, which has nothing to nest
  (error `nested_toc_unsupported_format`, its message now naming EPUB and PDF). `--format auto`
  with `--nested-toc` stays an EPUB on every profile, as ADR 0024 made it.
- **The outline is written by this tool**, into the `lopdf` document that `printpdf` returns,
  and the document is saved as `printpdf` saves it (compressed). `lopdf` is already part of the
  build through `printpdf`, at the same version and with no extra features, so no package or
  license notice is added. Titles are written as UTF-16 text strings, whatever their language.
- **The bookmarks of `ComicInfo.xml` are not used for the PDF.** The EPUB uses them as its chapters
  when there are any; a PDF takes its chapters from the folders only. They can be added the way
  the EPUB does it, by counting the pages a source page became, if someone needs them.

## Difference from KCC

| Difference | Why |
|---|---|
| A PDF has bookmarks, one for each chapter, and with `--nested-toc` one for each volume with its chapters under it; KCC's PDF has none | A long book with no way to reach a chapter is not a book to read. Nothing about the pages changes. |

## Consequences

- Every PDF has an outline, with at least one bookmark. A book of one chapter has a bookmark for it.
- A program that drives this tool can ask for a two-level PDF with `--nested-toc` and `--format pdf`.
  An older tool refuses it, so a program that uses it needs the release that carries this.
- `README.md`, the machine protocol's description of the error and the release notes say it.
- The EPUB and the CBZ are not changed. The PDF's pages, and the title and author in it, are not.
