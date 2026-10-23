"""Compares mangaprep's page processing with KCC's on your own manga, page by page.

The built-in comparisons use drawn pages. Before a release that changes processing, decoding or
book output, run this on real volumes too. They stay outside the repository.

    python tools/parity/real_pages.py --kcc ../kcc-reference --pages DIR [--pages DIR ...]
        [--profiles K11,KPW6,KS3,KoLC,K810,RmkPPMove] [--sweep [--sample 60]] [--webtoon DIR]

Each `--pages` folder is one book: its images, in natural order. Without `--sweep`, every profile
runs with the options KCC's window starts with on that device (manga style, margins and page
numbers cropped, spreads split and rotated, upscaling and colour as the device starts them, read
from the KCC checkout). `--sweep` runs every scenario of parity.py whose corpus is pages, spreads
or colour pages, on the first profile, over a sample of each book (every spread kept).
`--webtoon DIR` merges a folder of webtoon strips and cuts it, compared exactly, then compares
the cut pages through webtoon mode.

A page passes as in parity.py: the same background, pieces, sizes and grey-or-colour, and pixels
within its limits. The pages that differ most are listed so they can be looked at.
"""

import argparse
import os
from pathlib import Path
import re
import sys

import numpy as np
from natsort import os_sorted
from PIL import Image

import parity
from kcc_oracle import load_kcc

IMAGES = (".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp")
DEFAULT_PROFILES = "K11,KPW6,KS3,KoLC,K810,RmkPPMove"

Image.MAX_IMAGE_PIXELS = None


def window_defaults(kcc):
    """Upscaling and colour as KCC's window starts them on each device, from its profile table."""
    text = Path(kcc, "kindlecomicconverter", "KCC_gui.py").read_text(encoding="utf-8")
    return parse_window_defaults(text)


def parse_window_defaults(text):
    defaults = {}
    for body in re.findall(r"\{([^{}]*'Label':\s*'[^']+'[^{}]*)\}", text):
        label = re.search(r"'Label':\s*'([^']+)'", body).group(1)
        upscale = re.search(r"'DefaultUpscale':\s*(True|False)", body)
        colour = re.search(r"'ForceColor':\s*(True|False)", body)
        if upscale and colour:
            defaults[label] = (upscale.group(1) == "True", colour.group(1) == "True")
    if not defaults:
        raise SystemExit("could not read the device table of KCC's window")
    return defaults


def window_arguments(profile, defaults):
    """KCC's and parity_dump's arguments for the options KCC's window starts with."""
    upscale, colour = defaults[profile]
    kcc = ["-p", profile, "-m", "-r", "2", "-f", "CBZ"]
    dump = ["--manga", "--splitter=both", "--format=cbz"]
    if upscale:
        kcc.append("-u")
        dump.append("--upscale")
    if colour:
        kcc.append("--forcecolor")
        dump.append("--forcecolor")
    return kcc, dump


def pages_of(folder):
    return os_sorted([os.path.join(root, name) for root, _, names in os.walk(folder)
                      for name in names if name.lower().endswith(IMAGES)])


def sample(files, count):
    if count <= 0 or len(files) <= count:
        return files
    picked = {files[0]} | {files[int(i * len(files) / count)] for i in range(count)}
    for path in files:
        with Image.open(path) as image:
            if image.width > image.height:
                picked.add(path)
    return [path for path in files if path in picked]


def measure(files, kcc_dir, dump_dir):
    """parity.compare_pages, keeping each piece's difference for the list of worst pages."""
    report = parity.Report()
    parity.compare_pages(report, "", files, kcc_dir, dump_dir)
    kcc = parity.manifest_pages(kcc_dir, "kcc.json")
    ours = parity.manifest_pages(dump_dir, "mangaprep.json")
    pieces = []
    for path, theirs, mine in zip(files, kcc, ours):
        for a, b in zip(theirs["pieces"], mine["pieces"]):
            if a["size"] != b["size"]:
                continue
            x = parity.comparison_image(os.path.join(kcc_dir, a["file"]))
            y = parity.comparison_image(os.path.join(dump_dir, b["file"]))
            if x.mode == y.mode:
                difference = np.abs(np.asarray(x, dtype=np.int16) - np.asarray(y, dtype=np.int16))
                pieces.append((float(difference.mean()), int(difference.max()), path, b["role"]))
    return report, pieces


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--kcc", required=True, help="path to a KCC checkout")
    parser.add_argument("--pages", action="append", default=[], help="a folder of one book's pages; repeat")
    parser.add_argument("--profiles", default=DEFAULT_PROFILES, help=f"device profiles (default: {DEFAULT_PROFILES})")
    parser.add_argument("--sweep", action="store_true", help="every page scenario of parity.py, on the first profile")
    parser.add_argument("--sample", type=int, default=60, help="pages of each book for --sweep (0: all)")
    parser.add_argument("--webtoon", action="append", default=[], help="a folder of webtoon strips; repeat")
    parser.add_argument("--work", default=os.path.join(parity.REPO, "target", "parity-real"))
    parser.add_argument("--worst", type=int, default=10, help="how many of the most different pages to list")
    args = parser.parse_args()
    if not args.pages and not args.webtoon:
        parser.error("give --pages or --webtoon")

    kcc = os.path.abspath(args.kcc)
    version, kcc_cli, _, _ = load_kcc(kcc)
    profiles = args.profiles.split(",")
    defaults = window_defaults(kcc)
    unknown = [profile for profile in profiles if profile not in defaults]
    if unknown:
        parser.error(f"not a device of KCC's window: {', '.join(unknown)}")
    work = os.path.abspath(args.work)
    os.makedirs(work, exist_ok=True)
    oracle = [sys.executable, os.path.join(parity.HERE, "kcc_oracle.py"), kcc]

    print("building parity_dump ...")
    parity.run(["cargo", "build", "--release", "--locked", "-p", "mangaprep-core", "--example", "parity_dump"], cwd=parity.REPO)
    dump = os.path.join(parity.REPO, "target", "release", "examples", "parity_dump" + (".exe" if os.name == "nt" else ""))

    runs = []
    for folder in args.pages:
        book = os.path.basename(os.path.normpath(folder))
        files = pages_of(folder)
        if not files:
            parser.error(f"{folder} has no pages")
        if args.sweep:
            chosen = sample(files, args.sample)
            for name, key, kcc_extra, dump_extra, *bases in parity.SCENARIOS:
                if key in ("pages", "spreads", "colour"):
                    kcc_base, dump_base = parity.scenario_bases(profiles[0], bases)
                    runs.append((book, name, chosen, kcc_base + kcc_extra, dump_base + dump_extra))
        else:
            for profile in profiles:
                runs.append((book, f"window defaults [{profile}]", files, *window_arguments(profile, defaults)))

    failures, worst = [], []
    print(f"comparing against KCC {version}")
    for index, (book, name, files, kcc_args, dump_args) in enumerate(runs):
        kcc_dir, dump_dir = os.path.join(work, f"{index:03d}_kcc"), os.path.join(work, f"{index:03d}_mangaprep")
        list_file = os.path.join(work, f"{index:03d}_list.txt")
        Path(list_file).write_text("\n".join(files) + "\n", encoding="utf-8")
        parity.run(oracle + [kcc_dir, list_file] + kcc_args)
        profile = kcc_args[kcc_args.index("-p") + 1]
        lossless = [] if any(flag.startswith("--forcepng") for flag in dump_args) else ["--lossless"]
        parity.run([dump, dump_dir, list_file, profile] + dump_args + lossless)
        report, pieces = measure(files, kcc_dir, dump_dir)
        failures += [f"{book} / {name}{failure}" for failure in report.failures]
        worst += [(mean, peak, path, role, book, name) for mean, peak, path, role in pieces]
        largest = max((mean for mean, *_ in pieces), default=0.0)
        status = "ok  " if not report.failures else "FAIL"
        print(f"  {status} {book[:32]:32s} {name[:44]:44s} {len(files):4d} pages, largest mean difference {largest:.2f}", flush=True)

    for folder in args.webtoon:
        chapter = os.path.abspath(folder)
        book = os.path.basename(os.path.normpath(folder))
        for profile in profiles:
            options = kcc_cli.checkOptions(kcc_cli.makeParser().parse_args(["-p", profile, "-w", "-f", "EPUB", "unused-input"]))
            width, height = options.profileData[1]
            kcc_dir, dump_dir = os.path.join(work, f"webtoon_{profile}_kcc"), os.path.join(work, f"webtoon_{profile}_mangaprep")
            parity.run(oracle + [kcc_dir, "--webtoon-dir", chapter, str(width), str(height)])
            parity.run([dump, dump_dir, chapter, profile, "--webtoon-dir"])
            theirs = os_sorted(os.listdir(kcc_dir))
            mine = os_sorted(page for page in os.listdir(dump_dir) if page.startswith("page-"))
            same = lambda a, b: np.array_equal(np.asarray(Image.open(a).convert("RGB")), np.asarray(Image.open(b).convert("RGB")))
            if not same(kcc_dir + "_strip.png", os.path.join(dump_dir, "strip.png")):
                problem = "merged strips differ"
            elif len(theirs) != len(mine):
                problem = f"KCC cut {len(theirs)} pages, mangaprep {len(mine)}"
            elif not all(same(os.path.join(kcc_dir, a), os.path.join(dump_dir, b)) for a, b in zip(theirs, mine)):
                problem = "cut pages differ"
            else:
                problem = None
            if problem:
                failures.append(f"{book} / webtoon [{profile}]: {problem}")
            print(f"  {'ok  ' if not problem else 'FAIL'} {book[:32]:32s} webtoon cut [{profile}]{'':25s} {len(theirs):4d} pages, {problem or 'identical'}", flush=True)
            pages = [os.path.join(kcc_dir, page) for page in theirs]
            list_file = os.path.join(work, f"webtoon_{profile}_list.txt")
            Path(list_file).write_text("\n".join(pages) + "\n", encoding="utf-8")
            parity.run(oracle + [kcc_dir + "_pages", list_file, "-p", profile, "-w", "-f", "EPUB"])
            parity.run([dump, dump_dir + "_pages", list_file, profile, "--webtoon"])
            report, pieces = measure(pages, kcc_dir + "_pages", dump_dir + "_pages")
            failures += [f"{book} / webtoon pages [{profile}]{failure}" for failure in report.failures]
            largest = max((mean for mean, *_ in pieces), default=0.0)
            print(f"  {'ok  ' if not report.failures else 'FAIL'} {book[:32]:32s} webtoon pages [{profile}]{'':23s} {len(pages):4d} pages, largest mean difference {largest:.2f}", flush=True)

    if worst and args.worst:
        print(f"\nthe {args.worst} pages that differ most:")
        for mean, peak, path, role, book, name in sorted(worst, reverse=True)[:args.worst]:
            print(f"  mean {mean:6.2f}  max {peak:3d}  {role:11s} {name[:36]:36s} {path}")
    print()
    if failures:
        print(f"{len(failures)} difference(s) from KCC {version}:")
        for failure in failures:
            print("  -", failure)
        raise SystemExit(1)
    print(f"mangaprep matches KCC {version} on these pages")


if __name__ == "__main__":
    main()
