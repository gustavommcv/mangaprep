//! Resized-`.cbz` output: the processed pages re-zipped under the folders
//! their chapters were in, path and all (sanitized only enough to be safe zip
//! entry names).
//!
//! Unlike [`super::epub`], there's no TOC to build, so the chapters' folders
//! are the book's only structure — a resized CBZ is meant to be
//! human-browsable the same way the source `.cbz` was, and a reader that
//! orders pages by their names (MuPDF, so KOReader, does) must find them in
//! the order of the book. That is why a chapter keeps the whole path it had
//! and not just the name of its last folder: two volumes that each number
//! their chapters from `c001` would otherwise be shown chapter by chapter,
//! alternating between the volumes.

use super::Chapter;
use crate::error::{Error, Result};
use std::path::Component;

/// `--keepcomicinfo`: re-embeds the *original* `ComicInfo.xml` bytes
/// (unmodified — upstream doesn't rewrite it to reflect the resize either)
/// at the root of the output CBZ. `None` when the source had no
/// `ComicInfo.xml` or `--keepcomicinfo` wasn't requested.
///
/// `cover`, when given, goes in as `##cover.jpg` at the root — upstream's
/// name for it, chosen to sort ahead of every page so that readers show it
/// first. Upstream only adds one when the cover is not simply the first
/// page: a cover the user supplied, or one smart-cropped out of a wide
/// image.
pub fn build_cbz(
    chapters: &[Chapter],
    comic_info_xml: Option<&[u8]>,
    cover: Option<&[u8]>,
) -> Result<Vec<u8>> {
    let mut entries = Vec::new();

    if let Some(cover) = cover {
        entries.push(("##cover.jpg".to_string(), cover.to_vec()));
    }

    if let Some(xml) = comic_info_xml {
        entries.push(("ComicInfo.xml".to_string(), xml.to_vec()));
    }

    // A chapter's folder is its whole path in the input (see [`chapter_folder`]),
    // so two chapters can only share a folder name if they had the same path
    // or paths that sanitize to the same one. The first keeps its name
    // unchanged and a later one gets a disambiguating suffix, rather than the
    // two silently merging into one zip directory.
    let mut seen_folder_counts: std::collections::HashMap<String, usize> =
        std::collections::HashMap::new();
    for chapter in chapters {
        let base_folder = chapter_folder(chapter);
        let occurrence = seen_folder_counts.entry(base_folder.clone()).or_insert(0);
        *occurrence += 1;
        let safe_title = if *occurrence == 1 {
            base_folder
        } else {
            format!("{base_folder} ({occurrence})")
        };
        for (page_index, page) in chapter.pages.iter().enumerate() {
            let name = format!("{safe_title}/p{:04}.{}", page_index + 1, page.extension);
            entries.push((name, page.bytes.clone()));
        }
    }

    let extras = comic_info_xml.is_some() as usize + cover.is_some() as usize;
    if entries.len() == extras {
        return Err(Error::EmptyBook);
    }

    crate::archive::cbz::write_zip(&entries, true)
}

/// The folder a chapter's pages are written into: the path of the folder it
/// was in, relative to the input, one folder per level.
///
/// Only real folder names are kept. A name that would make a reader or an
/// extractor leave the archive (`..`, a root, a drive) is dropped, as is the
/// empty path of pages lying directly in the book, which is titled by the
/// chapter's own title instead.
fn chapter_folder(chapter: &Chapter) -> String {
    let levels: Vec<String> = chapter
        .relative_path
        .components()
        .filter_map(|level| match level {
            Component::Normal(name) => Some(name.to_string_lossy().replace('\\', "-")),
            _ => None,
        })
        .collect();
    if levels.is_empty() {
        chapter.title.replace(['/', '\\'], "-")
    } else {
        levels.join("/")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebook::Page;
    use std::path::{Path, PathBuf};

    fn chapter(title: &str, pages: usize) -> Chapter {
        Chapter {
            relative_path: PathBuf::from(title),
            title: title.to_string(),
            pages: (0..pages)
                .map(|_| Page {
                    extension: "jpg".to_string(),
                    bytes: b"fake-jpeg".to_vec(),
                    ..Default::default()
                })
                .collect(),
        }
    }

    #[test]
    fn empty_chapters_are_rejected() {
        assert!(matches!(build_cbz(&[], None, None), Err(Error::EmptyBook)));
    }

    #[test]
    fn empty_chapters_with_comicinfo_are_still_rejected() {
        // A lone ComicInfo.xml with no actual pages isn't a book.
        assert!(matches!(
            build_cbz(&[], Some(b"<ComicInfo/>"), None),
            Err(Error::EmptyBook)
        ));
    }

    #[test]
    fn builds_one_folder_per_chapter() {
        let chapters = vec![chapter("c001 - One", 2), chapter("c002 - Two", 1)];
        let bytes = build_cbz(&chapters, None, None).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        assert_eq!(
            names,
            vec![
                "c001 - One/p0001.jpg",
                "c001 - One/p0002.jpg",
                "c002 - Two/p0001.jpg",
            ]
        );
    }

    #[test]
    fn chapters_that_would_share_a_folder_get_disambiguated() {
        // Two chapters with the same path (or paths that sanitize to the same
        // folder name) -- the first keeps the plain name, the second must not
        // collide with it in the output zip.
        let chapters = vec![chapter("Extras", 1), chapter("Extras", 1)];
        let bytes = build_cbz(&chapters, None, None).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        assert_eq!(names, vec!["Extras/p0001.jpg", "Extras (2)/p0001.jpg"]);
    }

    fn nested_chapter(path: &str, level: u8) -> Chapter {
        let title = path.rsplit('/').next().unwrap();
        Chapter {
            relative_path: PathBuf::from(path),
            title: title.to_string(),
            pages: vec![Page {
                extension: "png".to_string(),
                bytes: vec![level],
                ..Default::default()
            }],
        }
    }

    fn entry_names(bytes: Vec<u8>) -> Vec<String> {
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect()
    }

    #[test]
    fn chapters_keep_the_folders_they_were_in() {
        let chapters = vec![
            nested_chapter("v001 - Vol.01/c001 - One", 0),
            nested_chapter("v002 - Vol.02/c001 - Two", 1),
        ];
        let names = entry_names(build_cbz(&chapters, None, None).unwrap());
        assert_eq!(
            names,
            vec![
                "v001 - Vol.01/c001 - One/p0001.png",
                "v002 - Vol.02/c001 - Two/p0001.png"
            ]
        );
    }

    #[test]
    fn two_volumes_numbered_from_the_same_chapter_are_read_in_book_order() {
        // What Mangabind writes for a whole series: every volume numbers its
        // chapters again from c001. A reader that sorts the pages by name,
        // as MuPDF does, must still show volume one before volume two.
        let chapters = vec![
            nested_chapter("v001 - Vol.01/c001 - A", 0),
            nested_chapter("v001 - Vol.01/c002 - B", 1),
            nested_chapter("v002 - Vol.02/c001 - C", 2),
            nested_chapter("v002 - Vol.02/c002 - D", 3),
        ];
        let bytes = build_cbz(&chapters, None, None).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        names.sort_by(|a, b| crate::natural_sort::compare_paths(Path::new(a), Path::new(b)));
        let levels: Vec<u8> = names
            .iter()
            .map(|name| {
                let mut page = Vec::new();
                std::io::Read::read_to_end(&mut archive.by_name(name).unwrap(), &mut page).unwrap();
                page[0]
            })
            .collect();
        assert_eq!(levels, vec![0, 1, 2, 3]);
    }

    #[test]
    fn the_same_chapter_name_in_two_volumes_needs_no_suffix() {
        let chapters = vec![
            nested_chapter("VolumeA/Extras", 0),
            nested_chapter("VolumeB/Extras", 1),
        ];
        let names = entry_names(build_cbz(&chapters, None, None).unwrap());
        assert_eq!(
            names,
            vec!["VolumeA/Extras/p0001.png", "VolumeB/Extras/p0001.png"]
        );
    }

    #[test]
    fn a_path_that_climbs_out_of_the_book_is_not_written() {
        let chapters = vec![
            nested_chapter("../../outside/c001", 0),
            nested_chapter("/rooted/c002", 1),
        ];
        let names = entry_names(build_cbz(&chapters, None, None).unwrap());
        assert_eq!(
            names,
            vec!["outside/c001/p0001.png", "rooted/c002/p0001.png"]
        );
    }

    #[test]
    fn pages_lying_directly_in_the_book_are_titled_by_the_chapter() {
        let chapters = vec![Chapter {
            relative_path: PathBuf::new(),
            title: "Untitled".to_string(),
            pages: vec![Page {
                extension: "png".to_string(),
                bytes: vec![0],
                ..Default::default()
            }],
        }];
        let names = entry_names(build_cbz(&chapters, None, None).unwrap());
        assert_eq!(names, vec!["Untitled/p0001.png"]);
    }

    #[test]
    fn slashes_in_the_title_of_loose_pages_are_sanitized() {
        // Pages lying directly in the book have no folder, so the chapter's
        // title is the folder's name -- and `/` in it would read back as a
        // subfolder.
        let chapters = vec![Chapter {
            relative_path: PathBuf::new(),
            title: "c001 - A/B".to_string(),
            pages: vec![Page {
                extension: "jpg".to_string(),
                bytes: b"fake-jpeg".to_vec(),
                ..Default::default()
            }],
        }];
        let names = entry_names(build_cbz(&chapters, None, None).unwrap());
        assert_eq!(names, vec!["c001 - A-B/p0001.jpg"]);
    }

    // Where a backslash is a separator (Windows) it never sits inside a name: it has already
    // split the path into levels by the time the writer sees it.
    #[cfg(unix)]
    #[test]
    fn a_backslash_in_a_folder_name_is_not_a_level() {
        let chapters = vec![nested_chapter("Vol 1/c001 - A\\B", 0)];
        let names = entry_names(build_cbz(&chapters, None, None).unwrap());
        assert_eq!(names, vec!["Vol 1/c001 - A-B/p0001.png"]);
    }

    #[test]
    fn keepcomicinfo_embeds_the_original_bytes_at_the_root() {
        let chapters = vec![chapter("c001 - One", 1)];
        let bytes = build_cbz(
            &chapters,
            Some(b"<ComicInfo><Series>Test</Series></ComicInfo>"),
            None,
        )
        .unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        assert_eq!(
            archive.by_index(0).unwrap().name().unwrap(),
            "ComicInfo.xml"
        );
        let mut contents = String::new();
        std::io::Read::read_to_string(
            &mut archive.by_name("ComicInfo.xml").unwrap(),
            &mut contents,
        )
        .unwrap();
        assert_eq!(contents, "<ComicInfo><Series>Test</Series></ComicInfo>");
    }

    #[test]
    fn a_cover_is_stored_under_upstreams_name_ahead_of_every_page() {
        let chapters = vec![Chapter {
            relative_path: PathBuf::from("c001"),
            title: "c001".to_string(),
            pages: vec![Page {
                extension: "jpg".to_string(),
                bytes: vec![1, 2, 3],
                ..Default::default()
            }],
        }];
        let bytes = build_cbz(&chapters, None, Some(b"cover bytes")).unwrap();
        let mut archive = zip::ZipArchive::new(std::io::Cursor::new(bytes)).unwrap();
        let mut names: Vec<String> = (0..archive.len())
            .map(|i| archive.by_index(i).unwrap().name().unwrap().to_string())
            .collect();
        names.sort();
        assert_eq!(names[0], "##cover.jpg");
        assert_eq!(names.len(), 2);

        // A cover alone is still an empty book.
        assert!(matches!(
            build_cbz(&[], None, Some(b"cover bytes")),
            Err(Error::EmptyBook)
        ));
    }
}
