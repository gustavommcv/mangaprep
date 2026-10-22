# 28. Decode JPEG pages with libjpeg-turbo, as KCC's Pillow does

Date: 2026-10-21

## Status

Proposed on 2026-10-21. The maintainer approves this record by merging the pull request that
carries it. It applies [ADR 0013](0013-follow-a-named-kcc-release.md) (a difference from KCC
12.0.0 is a defect unless it is recorded) to how a JPEG page is read, and adds the `mozjpeg`
dependency.

## Context

Every comparison with KCC had used pages whose pixels both tools read the same way: drawn PNG
pages, or JPEG pages near no threshold. A comparison on real manga (the volume 1 of five titles,
963 pages, and a webtoon chapter of 22 JPEG strips) ran each case twice, once from the files and
once from a copy decoded by Pillow and saved as PNG. From the PNG copy, every page, every crop and
every webtoon cut was identical to KCC's on six devices. From the JPEG files, almost all were, but:

- one page in about 960 was cropped one column narrower (a colour page with its margin at the
  threshold: 725 against 726 px wide on a Kindle Scribe 3, a one-pixel shift of the whole page
  on the devices that upscale);
- two of the 300 cuts of the webtoon chapter landed 8 px from KCC's.

The pages are processed alike; they are read differently. mangapress decoded JPEG with
`zune-jpeg` through the `image` crate. KCC opens pages with Pillow, which decodes JPEG with
libjpeg-turbo. Against Pillow's decoding of all 791 JPEG files of that corpus:

| Decoder | Files identical | Samples that differ | Largest difference |
|---|---|---|---|
| `zune-jpeg` (the `image` crate's) | 0 | 0.7–2.1% | 3 levels, 12 on one title |
| `jpeg-decoder` (pure Rust) | 0 | 1.8–2.9% | 3 levels |
| libjpeg-turbo, through the `mozjpeg` crate | 791 | 0% | 0 |

A level is invisible, but where a margin or a gap between panels sits at a threshold it decides
the pixel, and with it where the crop or the cut falls.

## Decision

- **A gray, YCbCr or RGB JPEG is decoded by libjpeg-turbo** through the `mozjpeg` crate, with
  libjpeg's defaults as Pillow uses them (the accurate integer transform, smooth chroma
  upsampling): `decode_jpeg_as_kcc` in `input.rs`, under every page, cover, spread and webtoon
  strip, since all of them are decoded there.
- **Its header is checked first, as every page's is** (ADR 0015): the pixel limit and the
  allocation it needs, by the `image` crate, before libjpeg-turbo sees the file.
- **Anything else stays with the `image` crate**, unchanged: a CMYK or YCCK JPEG, and any file
  libjpeg-turbo refuses. libjpeg reports an error by unwinding out of its C code, which
  `catch_unwind` stops there; the `image` crate then decodes the file or refuses it as before.
- **Built from C sources, without SIMD**: `mozjpeg-sys` with only its `unwinding` and `parallel`
  features. No assembler is needed, every target decodes alike, and the C path gives Pillow's
  pixels as above. The program stays one static binary with nothing to install (ADR 0001); what
  changes is that building it needs a C compiler, which each release runner has (MSVC on Windows,
  Xcode's clang for both macOS targets, `musl-tools` for Linux). CI builds the two release targets
  its test matrix does not (Linux on musl, Intel macOS cross-built on Apple Silicon) on every change,
  and runs the decoding tests on musl, so a C build that breaks is seen before a tag.
- **The webtoon comparison reads strips as the program does.** `parity_dump --webtoon-dir` opened
  the strips with `image::open`, so it compared the `image` crate's decoding, not the program's.
  Both now go through `webtoon::decode_strip_image`.
- **Licenses**: libjpeg-turbo's notice (the IJG's and its own BSD-3-Clause) and the build
  dependency `dunce` (CC0-1.0) are added to `about.toml` and `THIRD-PARTY-NOTICES.md`. The IJG's
  acknowledgement was already in the notices for `jpeg-encoder` (ADR 0021).

## Consequences

- From the JPEG files themselves, the comparison on real manga is identical to KCC's on every page,
  crop and webtoon cut it checked.
- A truncated JPEG is read as Pillow reads it when KCC asks for truncated images: what was not
  read is filled by libjpeg-turbo.
- Decoding is slower without SIMD (8.1 s against 6.1 s for the 704 megapixels of those 791 files),
  but it is a small part of a conversion: a whole volume to CBZ on the Kindle 11 took 1 to 5% longer
  (Uzumaki volume 1: 6.59 s against 6.50 s, median of three). SIMD would need an assembler on every
  runner; it can be added on its own if the time matters.
- `tools/parity` no longer needs to keep JPEG sources away from thresholds.
- Unit tests decode synthetic gray and 4:2:0 colour JPEGs to the exact pixels Pillow 12.3.0 gives
  (`tests/fixtures/libjpeg`, where `zune-jpeg` differed on 22% of their samples), keep a CMYK JPEG
  with the `image` crate, survive a JPEG libjpeg-turbo refuses, and read a truncated one at its
  full size.
