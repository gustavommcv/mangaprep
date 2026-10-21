# 29. A palette page is cropped between panels by its colours, not by its palette indices

Date: 2026-10-21

## Status

Proposed on 2026-10-21. The maintainer approves this record by merging the pull request that
carries it. It adds a difference kept on purpose to the table of
[ADR 0019](0019-differences-from-kcc-found-by-the-october-comparison.md), as FILE-4 is kept there.

## Context

The comparison with KCC 12.0.0 on real manga ([audit-2026-10-21-real-manga.md](../audit-2026-10-21-real-manga.md))
ran every page scenario of `tools/parity/parity.py` over real pages. All of them agreed except the
inter-panel crop (`--ipc 1` and `--ipc 2`, mangapress's `--ipc=horizontal` and `--ipc=both`) on one
title whose pages are palette PNG files: 11 of its 63 sampled pages came out of KCC dark and
unreadable, edges on a grey ground, and out of mangapress as drawn.

KCC's `crop_empty_inter_panel` (`kindlecomicconverter/inter_panel_crop_alg.py`) finds the empty
rows on a grey copy of the page, then removes them from `np.array(img)` of the page itself and
rebuilds it with `Image.fromarray`. For a palette image those arrays are palette indices: the page
comes back as its indices drawn as grey levels, and its palette is lost. Only pages whose palette
is not in grey order show it, which is why 11 of 63 did. The same page converted to grey comes out
of both tools identical.

[ADR 0020](0020-follow-kcc-on-the-differences-left-open.md) follows KCC even where a difference
is small. It does not ask for KCC's accidents: FILE-4 (a 16-bit grey PNG that KCC turns almost
white) is kept on purpose for the same reason.

## Decision

| ID | KCC 12.0.0 | mangapress | Why |
|---|---|---|---|
| IPC-1 | With `--ipc 1` or `--ipc 2`, a palette PNG page whose palette is not in grey order comes out as its palette indices drawn as grey: dark and unreadable | The page is cropped between panels and keeps its tones | KCC's result is an accident of turning a palette image into an array; nobody asks for it |

No case is added to `tools/parity/differences.py`: a row of the kept table does not need one.

## Consequences

- A difference KCC users may meet with palette pages and `--ipc` does not exist here.
- Upstream could be told; that is for the maintainer to decide.
