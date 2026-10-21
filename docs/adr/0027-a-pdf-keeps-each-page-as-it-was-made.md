# 27. A PDF keeps each page as it was made

Date: 2026-10-21

## Status

Proposed on 2026-10-21. The maintainer approves this record by merging the pull request that
carries it. It applies a row of [ADR 0013](0013-follow-a-named-kcc-release.md) (KCC places each
page's own image in its PDF) and replaces the part of
[ADR 0026](0026-a-pdf-has-bookmarks.md) that saved the document as `printpdf` saves it.

## Context

A comparison with KCC 12.0.0 on real manga (the volume 1 of five titles and a webtoon chapter,
through both command lines) found the pages of a PDF made by this tool unlike those of its CBZ or
EPUB, and unlike KCC's PDF:

- **Smaller images.** The PDF was saved with `printpdf`'s `PdfSaveOptions::default()`, which in
  0.12.8 turns on image optimisation with `max_image_size: "2MB"`: any page over about 2 MB of
  pixels was shrunk before it was stored and stretched back by the reader. A grey page of the
  Kindle Paperwhite 6 (1272×1696) was stored at 1254×1672, an upscaled page of the Kindle
  Scribe 3 (1986×2648) at 1254×1672, and a colour page of the Kobo Libra Colour (1264×1680) at
  725×964. Pages of 1072×1448 (Kindle 11) were not touched, which is why nothing had shown.
- **Larger files.** Each page, already a JPEG, was decoded and its pixels stored again with Flate:
  the same pixels, in a file 2.3 to 4 times the size of KCC's (Uzumaki volume 1: 216.1 MB against
  94.8 MB; the webtoon chapter: 148.7 MB against 36.9 MB).
- **Every page named every image.** `printpdf` gives all pages one resource dictionary holding all
  of the book's images; a page draws only its own, but a reader is handed all of them for each.

KCC places each page's own JPEG in its PDF with PyMuPDF. Nothing in `printpdf` was needed for a
book of raster pages: no text, no fonts, no drawing. It brought about 35 packages of its own (font
shaping, Brotli, JSON, geometry) for that.

## Decision

- **The PDF is written with `lopdf` directly**, which already wrote its outline (ADR 0026), and
  `printpdf` is removed.
- **A JPEG page is placed as it is**, with `DCTDecode`: one component as `DeviceGray`, three as
  `DeviceRGB`. It is neither decoded nor compressed again, so the PDF holds the pixels of the CBZ
  and the EPUB. Its size is read from its header and held to the same pixel limit as a decode
  (ADR 0015). A JPEG a reader could not show as it is (CMYK, 12-bit, lossless or arithmetic-coded),
  which this tool does not make, is decoded and stored as pixels.
- **Any other page (a PNG) is stored losslessly**, with Flate: a grey page as one channel, a
  colour page as three. Nothing is reduced to fit a budget.
- **Each page names only its own image**, and its size is the image's size in whole points.
- The document keeps what it had: the title and the author in its Info dictionary, the outline,
  `PageMode /UseOutlines`. It has no creation date (the one before was the epoch, there only so
  that the same input gives the same bytes, which it still does). Text strings are UTF-16, in
  hexadecimal.

## Consequences

- A PDF is as sharp as the CBZ and EPUB of the same book on every device, and about the size of
  KCC's (Uzumaki volume 1: 96.9 MB against 94.8 MB; the colour volume on the Kobo Libra Colour:
  78.1 MB against 75.4 MB, its 211 pages at full size; the webtoon: 36.9 MB against 36.9 MB).
- Making a PDF no longer decodes and compresses every page again: a volume took about 5 seconds
  where it took 17 to 25.
- `printpdf` and the packages only it used leave `Cargo.lock` and the license notices.
- Unit tests check that a JPEG page is the page's own bytes, that a colour page of 6.7 MB of pixels
  comes back exactly, that a grey page is one channel, that each page names one image, and the
  size of each page in points; the outline's tests are unchanged.
