// Each test binary uses some of the shared helpers, not all.
#[allow(dead_code)]
mod support;

use lopdf::{Document, Object};
use std::path::Path;
use std::process::Command;
use support::{binary, write_png};

/// What Mangabind's -combine mode produces: a volume folder wrapping ordinary chapter folders.
fn series(dir: &Path) {
    for (volume, chapter, pages) in [
        ("v001 - Vol.01", "c001 - One", [32u8, 64]),
        ("v001 - Vol.01", "c002 - Two", [96, 128]),
        ("v002 - Vol.02", "c001 - Three", [160, 192]),
    ] {
        let chapter_path = dir.join(volume).join(chapter);
        std::fs::create_dir_all(&chapter_path).expect("create chapter");
        for (index, gray) in pages.into_iter().enumerate() {
            write_png(&chapter_path.join(format!("p{:04}.png", index + 1)), gray);
        }
    }
}

fn convert(input: &Path, output: &Path, extra: &[&str]) -> std::process::Output {
    Command::new(binary())
        .arg(input)
        .args(["--profile", "KV", "--format", "pdf", "--quiet", "--output"])
        .arg(output)
        .args(extra)
        .output()
        .expect("run mangaprep")
}

/// The bookmarks of a PDF as indented lines, one for each, in the order a reader lists them: the
/// depth, the title and the page it opens.
fn bookmarks(path: &Path) -> Vec<String> {
    fn level(doc: &Document, first: Option<&Object>, depth: usize, out: &mut Vec<String>) {
        let mut next = first.and_then(|object| object.as_reference().ok());
        while let Some(id) = next {
            let item = doc.get_dictionary(id).unwrap();
            let Ok(Object::String(raw, _)) = item.get(b"Title") else {
                panic!("a bookmark has no title");
            };
            let units: Vec<u16> = raw[2..]
                .chunks(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            let Ok(Object::Array(dest)) = item.get(b"Dest") else {
                panic!("a bookmark opens no page");
            };
            let page = doc
                .get_pages()
                .into_iter()
                .find(|(_, page_id)| *page_id == dest[0].as_reference().unwrap())
                .map(|(number, _)| number)
                .unwrap();
            out.push(format!(
                "{}{} -> {page}",
                "  ".repeat(depth),
                String::from_utf16(&units).unwrap()
            ));
            level(doc, item.get(b"First").ok(), depth + 1, out);
            next = item.get(b"Next").ok().and_then(|o| o.as_reference().ok());
        }
    }
    let doc = Document::load(path).expect("the PDF opens");
    let mut lines = Vec::new();
    if let Ok(outlines) = doc.catalog().unwrap().get(b"Outlines") {
        let outlines = doc
            .get_dictionary(outlines.as_reference().unwrap())
            .unwrap();
        level(&doc, outlines.get(b"First").ok(), 0, &mut lines);
    }
    lines
}

#[test]
fn a_pdf_has_a_bookmark_for_each_chapter() {
    let fixture = tempfile::tempdir().unwrap();
    series(fixture.path());
    let output_dir = tempfile::tempdir().unwrap();
    let output = output_dir.path().join("series.pdf");

    let result = convert(fixture.path(), &output, &[]);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        bookmarks(&output),
        ["c001 - One -> 1", "c002 - Two -> 3", "c001 - Three -> 5"]
    );
}

#[test]
fn a_pdf_of_a_combined_series_nests_the_chapters_under_their_volumes() {
    let fixture = tempfile::tempdir().unwrap();
    series(fixture.path());
    let output_dir = tempfile::tempdir().unwrap();
    let output = output_dir.path().join("series.pdf");

    let result = convert(fixture.path(), &output, &["--nested-toc"]);

    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    assert_eq!(
        bookmarks(&output),
        [
            "v001 - Vol.01 -> 1",
            "  c001 - One -> 1",
            "  c002 - Two -> 3",
            "v002 - Vol.02 -> 5",
            "  c001 - Three -> 5",
        ]
    );
}

#[test]
fn a_cbz_has_no_bookmarks_to_nest_so_asking_for_them_is_refused() {
    let fixture = tempfile::tempdir().unwrap();
    series(fixture.path());
    let output_dir = tempfile::tempdir().unwrap();

    let result = Command::new(binary())
        .arg(fixture.path())
        .args([
            "--profile",
            "KV",
            "--format",
            "cbz",
            "--nested-toc",
            "--output",
        ])
        .arg(output_dir.path().join("series.cbz"))
        .output()
        .expect("run mangaprep");

    assert_eq!(result.status.code(), Some(1));
    assert!(String::from_utf8_lossy(&result.stderr)
        .contains("--nested-toc requires --format epub or --format pdf"));
}
