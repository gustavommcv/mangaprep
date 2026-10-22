"""Makes the same books with KCC's CLI and mangapress's, in CBZ, EPUB and PDF, and compares them.

Run on your own volumes before a release that changes processing, decoding or book output. They
stay outside the repository.

    python tools/parity/real_books.py --kcc ../kcc-reference --book FILE_OR_FOLDER [--book ...]
        [--profiles K11] [--formats cbz,epub,pdf] [--webtoon] [--mangapress PATH]

A book is a `.cbz` or a folder of chapter folders, as Mangabind writes a volume. Both tools get
the options KCC's window starts with on the device (see real_pages.py), the same title and the
same author. Compared, for each book and format: the pages in reading order, their sizes and
decoded pixels (within JPEG noise), the contents and metadata of an EPUB, the page sizes and
metadata of a PDF, and the size of the file.

KCC reads a `.cbz` with 7-Zip, which has to be on PATH (or installed in its usual folder on
Windows). Each KCC run gets a temporary folder of its own: KCC removes every KCC-* folder of the
one it is given.
"""

import argparse
import io
import os
from pathlib import Path
import re
import shutil
import subprocess
import sys
import zipfile
from xml.etree import ElementTree

import numpy as np
from natsort import os_sorted
from PIL import Image

import parity
from real_pages import window_defaults

IMAGES = (".png", ".jpg", ".jpeg", ".webp", ".gif", ".bmp")
# Mean difference of decoded pixels between two JPEG pages from the same processed page: the two
# encoders round apart (ADR 0021). Measured at most 0.44 on real grey pages.
GRAY_LIMIT = 1.0
COLOUR_LIMIT = 1.5
# How much larger one tool's file may be than the other's.
SIZE_RATIO = 1.15
AUTHOR = "Real manga comparison"


def seven_zip_path():
    found = shutil.which("7z")
    if found:
        return None
    usual = Path(os.environ.get("ProgramFiles", r"C:\Program Files"), "7-Zip")
    if (usual / "7z.exe").exists():
        return str(usual)
    raise SystemExit("KCC needs 7-Zip on PATH to read a .cbz")


def run(command, env=None):
    result = subprocess.run(command, capture_output=True, text=True, encoding="utf-8", errors="replace", env=env)
    if result.returncode != 0:
        raise SystemExit(f"command failed: {' '.join(command)}\n{result.stdout[-2000:]}\n{result.stderr[-2000:]}")


def window_flags(profile, defaults):
    upscale, colour = defaults[profile]
    kcc = ["-p", profile, "-m", "-r", "2"] + (["-u"] if upscale else []) + (["--forcecolor"] if colour else [])
    ours = ["--profile", profile, "--manga-style", "--splitter=both"] + (["--upscale"] if upscale else []) + (["--forcecolor"] if colour else [])
    return kcc, ours


def read_cbz(path):
    with zipfile.ZipFile(path) as archive:
        names = os_sorted([name for name in archive.namelist() if name.lower().endswith(IMAGES)])
        return {"pages": [(name, archive.read(name)) for name in names], "meta": {}, "contents": []}


def read_epub(path):
    ns = {"opf": "http://www.idpf.org/2007/opf", "dc": "http://purl.org/dc/elements/1.1/",
          "c": "urn:oasis:names:tc:opendocument:xmlns:container", "n": "http://www.daisy.org/z3986/2005/ncx/"}
    with zipfile.ZipFile(path) as archive:
        container = ElementTree.fromstring(archive.read("META-INF/container.xml"))
        package_path = container.find(".//c:rootfile", ns).get("full-path")
        base = os.path.dirname(package_path)
        package = ElementTree.fromstring(archive.read(package_path))
        join = lambda folder, href: os.path.normpath(os.path.join(folder, href)).replace("\\", "/")
        manifest = {item.get("id"): item for item in package.find("opf:manifest", ns)}
        pages = []
        for reference in package.find("opf:spine", ns):
            document = join(base, manifest[reference.get("idref")].get("href"))
            for source in re.findall(r'<img[^>]+src="([^"]+)"', archive.read(document).decode("utf-8")):
                image = join(os.path.dirname(document), source)
                pages.append((image, archive.read(image)))
        # The centred spread property is written as the standard says (ADR 0018), not as KCC does.
        spreads = sorted((reference.get("properties") or "").replace("rendition:", "")
                         for reference in package.find("opf:spine", ns))
        meta = {
            "title": [e.text for e in package.findall(".//dc:title", ns)],
            "creator": [e.text for e in package.findall(".//dc:creator", ns)],
            "language": [e.text for e in package.findall(".//dc:language", ns)],
            "spreads": {value: spreads.count(value) for value in set(spreads)},
            "cover": [i.get("href") for i in manifest.values() if "cover-image" in (i.get("properties") or "")],
        }
        contents = []
        ncx = next((i for i in manifest.values() if i.get("media-type") == "application/x-dtbncx+xml"), None)
        if ncx is not None:
            tree = ElementTree.fromstring(archive.read(join(base, ncx.get("href"))))
            contents = [point.find("n:navLabel/n:text", ns).text for point in tree.iter(f"{{{ns['n']}}}navPoint")]
    return {"pages": pages, "meta": meta, "contents": contents}


def read_pdf(path):
    import pymupdf

    document = pymupdf.open(path)
    pages = []
    for page in document:
        drawn = [info["xref"] for info in page.get_image_info(xrefs=True) if info.get("xref")]
        pages.append((f"page {page.number + 1}", document.extract_image(drawn[0])["image"] if drawn else None))
    meta = {"title": document.metadata.get("title"), "author": document.metadata.get("author"),
            "page sizes": sorted({(round(page.rect.width), round(page.rect.height)) for page in document})}
    return {"pages": pages, "meta": meta, "contents": []}


READERS = {"cbz": read_cbz, "epub": read_epub, "pdf": read_pdf}


def compare(theirs, mine, fmt):
    problems, largest = [], 0.0
    if len(theirs["pages"]) != len(mine["pages"]):
        return [f"pages: KCC {len(theirs['pages'])}, mangapress {len(mine['pages'])}"], largest
    for number, ((_, a), (_, b)) in enumerate(zip(theirs["pages"], mine["pages"]), 1):
        if a is None or b is None:
            problems.append(f"page {number} has no image")
            continue
        with Image.open(io.BytesIO(a)) as x, Image.open(io.BytesIO(b)) as y:
            if x.size != y.size:
                problems.append(f"page {number}: KCC {x.size}, mangapress {y.size}")
                continue
            mode = "RGB" if "RGB" in (x.mode, y.mode) else "L"
            difference = np.abs(np.asarray(x.convert(mode), dtype=np.int16) - np.asarray(y.convert(mode), dtype=np.int16)).mean()
        largest = max(largest, float(difference))
        if difference > (COLOUR_LIMIT if mode == "RGB" else GRAY_LIMIT):
            problems.append(f"page {number}: pixels differ by {difference:.2f} levels on average")
    for key in theirs["meta"]:
        if theirs["meta"][key] != mine["meta"].get(key):
            problems.append(f"{key}: KCC {theirs['meta'][key]}, mangapress {mine['meta'].get(key)}")
    if fmt == "epub" and theirs["contents"] != mine["contents"]:
        problems.append(f"contents: KCC {theirs['contents'][:3]}, mangapress {mine['contents'][:3]}")
    return problems, largest


def main():
    parser = argparse.ArgumentParser(description=__doc__.split("\n")[0])
    parser.add_argument("--kcc", required=True, help="path to a KCC checkout")
    parser.add_argument("--book", action="append", required=True, help="a .cbz or a folder of chapters; repeat")
    parser.add_argument("--profiles", default="K11")
    parser.add_argument("--formats", default="cbz,epub,pdf")
    parser.add_argument("--webtoon", action="store_true", help="webtoon mode on both sides")
    parser.add_argument("--mangapress", help="the mangapress executable (default: build the release one)")
    parser.add_argument("--work", default=os.path.join(parity.REPO, "target", "parity-real-books"))
    args = parser.parse_args()

    kcc = os.path.abspath(args.kcc)
    defaults = window_defaults(kcc)
    env = dict(os.environ)
    seven_zip = seven_zip_path()
    if seven_zip:
        env["PATH"] = seven_zip + os.pathsep + env["PATH"]
    mangapress = os.path.abspath(args.mangapress) if args.mangapress else None
    if not mangapress:
        print("building mangapress ...")
        run(["cargo", "build", "--release", "--locked", "-p", "mangapress-cli"])
        mangapress = os.path.join(parity.REPO, "target", "release", "mangapress" + (".exe" if os.name == "nt" else ""))
    work = os.path.abspath(args.work)
    os.makedirs(work, exist_ok=True)

    failures, index = [], 0
    for book in args.book:
        book = os.path.abspath(book)
        title = Path(book).stem
        for profile in args.profiles.split(","):
            kcc_flags, our_flags = window_flags(profile, defaults)
            webtoon = (["-w"], ["--webtoon"]) if args.webtoon else ([], [])
            for fmt in args.formats.split(","):
                index += 1
                base = os.path.join(work, f"{index:03d}")
                kcc_dir, temporary, ours = base + "_kcc", base + "_tmp", base + f"_mangapress.{fmt}"
                for folder in (kcc_dir, temporary):
                    shutil.rmtree(folder, ignore_errors=True)
                    os.makedirs(folder)
                run([sys.executable, os.path.join(kcc, "kcc-c2e.py"), *kcc_flags, *webtoon[0], "-f", fmt.upper(),
                     "-t", title, "-a", AUTHOR, "-o", kcc_dir, book],
                    env=dict(env, TEMP=temporary, TMP=temporary, TMPDIR=temporary))
                run([mangapress, book, *our_flags, *webtoon[1], "--format", fmt, "--output", ours,
                     f"--title={title}", f"--author={AUTHOR}"])
                theirs_path = max((os.path.join(kcc_dir, name) for name in os.listdir(kcc_dir)), key=os.path.getsize)
                problems, largest = compare(READERS[fmt](theirs_path), READERS[fmt](ours), fmt)
                sizes = os.path.getsize(theirs_path), os.path.getsize(ours)
                if max(sizes) > SIZE_RATIO * min(sizes):
                    problems.append(f"file size: KCC {sizes[0] / 1e6:.1f} MB, mangapress {sizes[1] / 1e6:.1f} MB")
                failures += [f"{title} [{profile} {fmt}]: {problem}" for problem in problems]
                print(f"  {'ok  ' if not problems else 'FAIL'} {title[:40]:40s} {profile:9s} {fmt:4s} "
                      f"largest mean difference {largest:.2f}, KCC {sizes[0] / 1e6:6.1f} MB, mangapress {sizes[1] / 1e6:6.1f} MB", flush=True)

    print()
    if failures:
        print(f"{len(failures)} difference(s) from KCC:")
        for failure in failures:
            print("  -", failure)
        raise SystemExit(1)
    print("mangapress makes the same books as KCC")


if __name__ == "__main__":
    main()
