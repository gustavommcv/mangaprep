// Each test binary uses some of the shared helpers, not all.
#[allow(dead_code)]
mod support;

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use support::{binary, write_png};

/// A `.cbz` of what `mangabind -combine` writes for a whole series: one folder for each volume and,
/// in it, one for each chapter, numbered again from `c001` in every volume.
fn combined_series(dir: &Path, chapters: &[&str]) -> PathBuf {
    let path = dir.join("Series.cbz");
    let mut archive = zip::ZipWriter::new(std::fs::File::create(&path).unwrap());
    for (index, chapter) in chapters.iter().enumerate() {
        let page = dir.join(format!("page{index}.png"));
        write_png(&page, 40 + 20 * index as u8);
        archive
            .start_file(
                format!("{chapter}/p0001.png"),
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
        archive.write_all(&std::fs::read(&page).unwrap()).unwrap();
    }
    archive.finish().unwrap();
    path
}

fn converted_names(book: &Path, output: &Path) -> Vec<String> {
    let result = Command::new(binary())
        .arg(book)
        .args(["--profile", "KV", "--format", "cbz", "--quiet", "--output"])
        .arg(output)
        .output()
        .expect("run mangaprep");
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stderr)
    );
    let archive = zip::ZipArchive::new(std::fs::File::open(output).unwrap()).unwrap();
    archive
        .file_names()
        .map(|name| name.unwrap().into_owned())
        .collect()
}

/// A reader that orders the pages by their names, as KOReader's MuPDF does, must find them in the
/// order of the book: the names in the archive, sorted, are the names in the order they were
/// written, and each chapter is still in its volume's folder.
fn assert_read_in_book_order(chapters: &[&str]) {
    let work = tempfile::tempdir().unwrap();
    let book = combined_series(work.path(), chapters);
    let names = converted_names(&book, &work.path().join("Series (converted).cbz"));

    let folders: Vec<&str> = names
        .iter()
        .map(|name| name.rsplit_once('/').expect("a page in a folder").0)
        .collect();
    assert_eq!(folders, chapters);

    let mut sorted = names.clone();
    sorted.sort();
    assert_eq!(sorted, names);
}

#[test]
fn volumes_that_number_their_chapters_again_are_read_in_book_order() {
    assert_read_in_book_order(&[
        "v001 - Vol.01/c001 - One",
        "v001 - Vol.01/c002 - Two",
        "v002 - Vol.02/c001 - Three",
        "v002 - Vol.02/c002 - Four",
    ]);
}

#[test]
fn untitled_chapters_with_the_same_name_in_every_volume_are_read_in_book_order() {
    assert_read_in_book_order(&[
        "v001 - Vol.01/c001",
        "v001 - Vol.01/c002",
        "v002 - Vol.02/c001",
        "v002 - Vol.02/c002",
    ]);
}
