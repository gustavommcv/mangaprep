# 30. Follow KCC 12.1.1

Date: 2026-10-23

## Status

Accepted on 2026-10-23. The maintainer decided it ("include everything KCC did, following the
12.1.1 tag"), and approves it by merging the pull request that carries it. It changes the
reference of [ADR 0013](0013-follow-a-named-kcc-release.md) from 12.0.0 to 12.1.1.

## Context

KCC published 12.1.1 on 2026-10-22, after 12.1.0, which it withdrew for a bug of its own in MOBI
output. Between 12.0.0 and 12.1.1 there are 39 commits, and two files that mangapress follows:
`image.py` and `comic2ebook.py`. The release was read, not only compared: each change was traced to
its pull request and the issue behind it, and each was measured in mangapress.

The parity check (ADR 0013) passed against 12.1.1 as it stood (468 checks, and 130 real pages of
three volumes that matched both releases, at most 0.15 gray levels apart on average), and that was no
reason to stop: the corpus had no 16-bit page and no scan saved as RGB, which are the two inputs KCC
changed how it reads.

What 12.1.1 changed, and whether mangapress had the same fault:

| KCC 12.1.1 | The fault | mangapress |
|---|---|---|
| Kobo names (PRs 1465, 1466; issues 919, 613) | A `!` in the name of a KEPUB crashes the Kobo's reader, and spaces and hyphens were dropped from the name. 12.0.0 skipped the safe renaming when an output folder was given. | Had it, in the way that mattered: with `--output <folder>`, which the desktop app always passes, `Dr. Stone! Vol.01 100%.cbz` made `Dr. Stone! Vol.01 100%.kepub.epub`. |
| 16-bit gray input (PR 1469; Pillow issue 3011) | Pillow keeps 16-bit gray as `I;16` and its conversion clips: the page came out white. | Did not: a 16-bit gradient came out as a gradient. It rounded `v / 257`; KCC now keeps the high byte. |
| A page that is not color is made gray before gamma (PR 1461) | Gamma on each channel and then the conversion differs from the conversion and then the gamma on a pixel that is only nearly neutral. Only a custom gamma shows it, and the author reports no visible difference. | Had the order of 12.0.0. |
| Fixed resolutions, and names of the profiles (PRs 1462, 1464; issues 760, 896, 945) | BOOX and other devices with no profile; the 2026 Kindles and the Clara BW were not named. | Had none of them. |

Not in the table, and not followed, for the reasons of ADR 0013: the translations of KCC's window
(Chinese, Japanese, Korean, Spanish), the renamed output option of its window, the packaging of
its command line without Qt, and everything about MOBI (the page-turn animation, and the 12.1.0
bug of the renaming code). Two open KCC issues were read and do not apply: a duplicate cover when
turning pages (#1458: the cover of an EPUB made here is metadata and the book opens on page 1) and
the height limit of a webtoon (#1455: the limit here is the area, and the error already says to
use separate chapter folders).

## Decision

**The reference is KCC 12.1.1**, commit `745ea3b`, in `REFERENCE_KCC`, the parity workflow, and the
documents that name it. The EPUB package, which the check does not cover and which unit tests pin
with values taken from KCC's own functions at 12.0.0, is unchanged between the two releases: the
diff touches neither the package nor the cover.

Each change in the table is followed:

- **Kobo names.** The EPUB of a Kobo profile (kepub or not, as long as the resolution is not a
  custom one) gets a derived name in which each of `/ \ ? % * : ; | " ' > < $ !` becomes `_`, one
  for one, and nothing else changes (spaces, dots and hyphens stay). It applies to a file or a
  folder, with an output folder or without. A name that was asked for is kept as asked. Up to now it
  collapsed runs of non-word characters and only applied beside the input.
- **16-bit pages** are read as the high byte of each sample, for gray, gray with alpha, color and
  color with alpha, as KCC 12.1.1 reads the gray and Pillow reads the others.
- **A page that is not color is made gray before the gamma**; a page in color that ends up gray, and
  a color page kept in color, keep the gamma on each channel first.
- **Profiles.** `K11`, `KPW6` and `KoC` have the names KCC gives them (`Kindle 11/2026`,
  `Kindle Paperwhite 6/2026`, `Kobo Clara HD/Kobo Clara 2E/Kobo Clara BW`), and the seven fixed
  resolutions (`2400x3200` down to `1072x1448`) follow `OTHER` in the order KCC lists them. A fixed
  resolution is a screen of that size and nothing else: it belongs to no family, as `OTHER` does not.
- **The comparison** has a 16-bit page and a scan saved as RGB in its corpus, and a stand-in for the
  Qt module KCC 12.1.1 imports even without a window, so that the oracle runs on it.

## Consequences

A book made for a Kobo from a title with one of those characters has a name the Kobo can open, with
the spaces it had. A person with a BOOX or another 300 ppi screen has a profile for it. The deliberate
differences of ADR 0013 stand as they were, except that the name an existing output is given when it
would be overwritten (`… (mangapress)`) is its own and not KCC's `_kcc0`, as before.

Against the code of 0.8.3 the new corpus fails where it should and not elsewhere. The scan saved as
RGB under a custom gamma fails (1.24 gray levels apart on average, the limit being 1.0). The 16-bit
page does not: it was 0.19 levels apart, which the limit lets through, and it is exact (0.00) with the
change; the unit tests of the decoding are what fail without it. With the change, all 502 checks pass.
A change that moves pixels still has to pass the check before it is merged.
