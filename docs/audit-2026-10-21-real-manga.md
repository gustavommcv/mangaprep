# Real-manga comparison with KCC 12.0.0 — 21 October 2026

Every earlier comparison with KCC ran on drawn pages ([audit-2026-10.md](audit-2026-10.md): "Parity ran on a drawn corpus, not on real scans"). This one ran on real manga, through the options Mangabound actually sends, before the first signed releases.

- **Versions:** mangapress 0.8.2 (`cc581ab`, the release binary and the `parity_dump` example built from the same commit), mangabind 0.7.2, Mangabound `main` (`01172fb`, its starting options), KCC 12.0.0 (still the latest release) with Pillow 12.3.0 and numpy 2.5.2 as in CI, and its real PyMuPDF and MozJPEG dependencies.
- **Machine:** Windows 11, 12 threads.
- **Corpus:** volume 1 of five titles and one webtoon chapter. The scans stayed on the maintainer's machine; nothing of them is in this repository.

## What was done about it

| Finding | Status |
|---|---|
| REAL-1, REAL-2: the PDF | Fixed in [PR #67](https://github.com/gustavommcv/mangapress/pull/67) ([ADR 0027](adr/0027-a-pdf-keeps-each-page-as-it-was-made.md)): every page at full size, each its own JPEG; on the same books the PDF is about the size of KCC's |
| REAL-3: the JPEG decoder | Fixed in [PR #68](https://github.com/gustavommcv/mangapress/pull/68) ([ADR 0028](adr/0028-decode-jpeg-pages-as-kccs-pillow-does.md)): from the JPEG files themselves, every page, crop and webtoon cut checked is identical to KCC's |
| REAL-4: KCC's inter-panel crop on palette pages | Kept on purpose, [ADR 0029](adr/0029-a-palette-page-is-cropped-between-panels-by-its-colours.md) |

The comparison can be run again with `tools/parity/real_pages.py` and `tools/parity/real_books.py` (see [Running it again](#running-it-again)).

| Title | Pages | Source | What it exercises |
|---|---|---|---|
| Chainsaw Man | 182 | palette PNG, some JPEG | black and white, screentone, palette images |
| Chainsaw Man (colour) | 203 | colour JPEG inside chapter CBZs | colour pages, grey output from colour, colour output |
| Chainsaw Man (low resolution) | 182 | the first title reduced to 490×715 JPEG | upscaling |
| Uzumaki | 213 | grey JPEG | heavy blacks, dark tones |
| Kimi no Na wa (doujinshi) | 183 | JPEG | a second scan source |
| Tower of God, chapter 1 | 22 strips, 1241×501 209 px in all | colour JPEG | webtoon merging and cutting |

## Method

Each question was answered once, where it is cheapest:

1. **Page processing**, without either codec: KCC's own code through `tools/parity/kcc_oracle.py` against `parity_dump --lossless`, page by page, with the per-page measures of `tools/parity/parity.py`.
   - Mangabound's starting options for each device (KCC's window defaults: manga style, crop margins and page numbers, split and rotate spreads, upscale and colour as the device starts them; read from KCC's own profile table, they match Mangabound's lists device for device) on K11 (the maintainer's Kindle), KPW6 (Mangabound's starting device), KS3, KoLC, K810 and RmkPPMove.
   - Every scenario of `parity.py` whose corpus is pages, spreads or colour pages, on K11, over 60 pages of each title plus every spread: 36 scenarios × 5 titles.
   - Webtoon: the merged strip, its cut pages and those pages through webtoon mode, on K11 and KoLC.
2. **Books**, through both real command lines: CBZ, EPUB and PDF of every title on K11, of the colour title on KoLC, and of the webtoon on K11. Compared: pages in reading order, their sizes and decoded pixels, file sizes, contents, metadata, cover.
3. **The app's path:** mangabind's volume of each title against the pages KCC would read from the chapter folders.

Every run was repeated from a copy of the corpus decoded by Pillow and saved as PNG, so that both tools read the same pixels. That separates what each tool does from how each decodes a JPEG.

## Results

| Layer | Result |
|---|---|
| Page processing, app defaults, 6 devices × 5 titles (963 pages) | Identical to KCC (largest mean difference 0.00) from the same pixels. From the JPEG files: within 0.02 levels on 95% of pages, except one page (REAL-3) |
| Every scenario on real pages (180 runs) | 178 identical; 2 differ because of a KCC bug (REAL-4) |
| Webtoon (300 pages on K11, 253 on KoLC) | Merged strip and every cut page identical from the same pixels; from the JPEG strips, two cuts land 8 px apart (REAL-3) |
| mangabind's volumes (5 titles) | Same bytes, same order as the pages KCC reads from the chapter folders |
| CBZ and EPUB books (16) | Same pages, sizes, contents, title, author, language, cover and file size; pixels within JPEG noise (≤ 0.44 levels mean) |
| PDF books (7) | Same pages, pixels and metadata; 2.3 to 4 times larger, and reduced in resolution on large or colour pages (REAL-1, REAL-2) |
| Speed (not controlled: three runs at a time) | CBZ and EPUB usually several times faster than KCC's CLI (1.3 to 9 times); PDF about as fast or slower, since each page is decoded and stored again (REAL-2) |

The EPUB's centred spreads are `rendition:page-spread-center` where KCC writes `page-spread-center`: decided in ADR 0018, not a finding.

## Findings

### REAL-1. A PDF page is stored at a lower resolution than the page when it is over about 2 MB of pixels

**Class:** defect. **Priority:** High. **Effort:** S. **Status:** fixed in PR #67.

**Where:** `crates/mangapress-core/src/ebook/pdf.rs`, which saves with `PdfSaveOptions::default()`. In printpdf 0.12.8 that default (`serialize.rs:64`) turns on image optimisation with `max_image_size: "2MB"`, and `RawImage::optimize` shrinks any image over it.

**Evidence:** pages and the image each one draws:

| Device | Page | Image in the PDF |
|---|---|---|
| K11, grey | 1072×1448 | 1072×1448 |
| KPW6 (Mangabound's starting device), grey, upscaled | 1272×1696 | 1254×1672 |
| KS3, grey, upscaled | 1986×2648 | 1254×1672 |
| KoLC, colour | 1264×1680 | 725×964 |

Every image lands at about 2 097 152 bytes of raw pixels. The reader stretches it back to the page, so the page is softer than the CBZ or EPUB of the same book, and much softer on a Scribe or in colour.

**Direction:** save with image optimisation off. With REAL-2, write the page's own JPEG.

### REAL-2. A PDF is 2.3 to 4 times the size of KCC's, and every page lists every image

**Class:** defect against ADR 0013 (KCC embeds the page's JPEG). **Priority:** Medium. **Effort:** M. **Status:** fixed in PR #67.

**Where:** the same file. Each page's JPEG is decoded and its pixels stored again with Flate (`RawImage::from_dynamic_image`); printpdf also puts every image in one resource dictionary shared by all pages.

**Evidence:** on K11: Chainsaw Man 127.9 MB against KCC's 63.5; Uzumaki 216.1 against 94.8; the low-resolution copy 146.1 against 52.2; the webtoon 148.7 against 36.9. The pixels are those of the JPEG page, so nothing is gained. The page dictionaries of a 190-page book each name 190 images; readers draw only the one in the content stream.

**Direction:** embed each page's JPEG as it is (`DCTDecode`), and PNG pages as they are, as KCC does; give each page its own resources.

### REAL-3. JPEG files are decoded differently from KCC, which moves a crop or a webtoon cut now and then

**Class:** difference from the reference (ADR 0013). **Priority:** Medium. **Effort:** M. **Status:** fixed in PR #68, which also found that `parity_dump --webtoon-dir` read strips with the `image` crate instead of the program's decoder.

**Where:** mangapress decodes JPEG through the `image` crate's `zune-jpeg` 0.5.15. KCC decodes it through Pillow, that is libjpeg-turbo.

**Evidence:** against Pillow's decoding of all 791 JPEG files of the corpus:

| Decoder | Files identical | Samples that differ | Largest difference |
|---|---|---|---|
| `zune-jpeg` (today) | 0 | 0.7–2.1% | 3 levels (12 on the low-resolution copy) |
| `jpeg-decoder` | 0 | 1.8–2.9% | 3 levels |
| libjpeg-turbo through the `mozjpeg` crate | 791 | 0% | 0 |

A difference of a level is invisible, but where a margin or a gap between panels is at the threshold it decides the pixel. One page in about 960 was cropped one column narrower (Chainsaw Man colour, chapter 3, page 23: 725 against 726 px on KS3, a one-pixel shift on upscaled devices), and two of 300 webtoon cuts moved by 8 px. From the same pixels, all of them agree.

**Direction:** decode JPEG with libjpeg-turbo's decoder through the `mozjpeg` crate (built from C sources with `cc`, without cmake or nasm; BSD/IJG/zlib licences, compatible with ADR 0007). Every release target then needs a C compiler; check the musl and macOS builds before deciding.

### REAL-4. KCC breaks the inter-panel crop on palette PNG pages; mangapress does not

**Class:** a KCC defect; keep the difference. **Priority:** Low (record). **Effort:** S. **Status:** kept, ADR 0029.

**Where:** KCC's `kindlecomicconverter/inter_panel_crop_alg.py` reads the image with `np.array(img)` (line 29) and rebuilds it with `Image.fromarray` (line 43). For a palette image those are palette indices, so the page comes out as its indices drawn as grey.

**Evidence:** with `--ipc 1` or `--ipc 2`, 11 of the 63 sampled Chainsaw Man pages (palette PNG) came out dark and unreadable from KCC and correct from mangapress. The same page converted to grey comes out identical from both tools.

**Direction:** keep it on purpose, as FILE-4 of ADR 0019 is kept. It could also be reported upstream.

## Running it again

Before a release that changes processing, decoding or book output, on volumes kept outside the
repository (one folder of pages per book for `real_pages.py`; a `.cbz` or a folder of chapters per
book for `real_books.py`):

```sh
python -m pip install -r tools/parity/requirements-real.txt
python tools/parity/real_pages.py --kcc ../kcc-reference --pages VOLUME1 --pages VOLUME2 --webtoon CHAPTER
python tools/parity/real_pages.py --kcc ../kcc-reference --pages VOLUME1 --sweep
python tools/parity/real_books.py --kcc ../kcc-reference --book VOLUME1.cbz --profiles K11,KoLC
```

## Not covered

- How the books look on a device. The maintainer reads samples on a Kindle (2024, profile K11).
- KCC's window itself. Mangabound's starting options were matched to KCC's CLI flags by reading both, not by driving the window.
- Books on devices other than K11 and KoLC, combined series (`--nested-toc` has no KCC counterpart), and options Mangabound offers that change only the book (`--spreadshift`, `--onepagelandscape`, covers), which `parity_dump` does not take.
