//! What the contents of a book are made of, before a format writes them: what a chapter is
//! called, and which volume it belongs to. The EPUB writes them as its navigation and the PDF as
//! its bookmarks, so the two agree on the names and on the volumes.

use super::Chapter;

/// What the contents call a chapter. Pages lying directly in the book have no folder to name
/// them, so the book's own title stands in, as upstream does; a bookmark has no pages and keeps
/// the name it was given.
pub(super) fn chapter_label<'a>(chapter: &'a Chapter, book_title: &'a str) -> &'a str {
    if chapter.relative_path.as_os_str().is_empty() && !chapter.pages.is_empty() {
        book_title
    } else {
        &chapter.title
    }
}

/// One volume's worth of already-flat chapters, grouped for a nested table of contents. Built
/// purely from each [`Chapter`]'s existing `relative_path` - its parent-of-parent directory is the
/// volume, matching how Mangabind's `-combine` mode nests `<volume dir>/<chapter dir>/...` (see
/// `docs/adr/0005-mangabind-contract.md` for why chapters are already grouped by full path, and
/// `docs/adr/0012-nested-toc-for-combined-volumes.md` for this one more level). No change to
/// [`crate::ebook::group_into_chapters`] or the [`Chapter`] type itself was needed for this.
///
/// What goes with each chapter, `T`, is whatever the format needs to point at it: the EPUB's href
/// of its first page, the PDF's number of it.
pub(super) struct VolumeGroup<'a, T> {
    pub(super) title: String,
    pub(super) chapters: Vec<(&'a Chapter, T)>,
}

/// The chapters grouped by volume, in the order the volumes first appear, each volume's chapters
/// in the order they were given.
pub(super) fn group_by_volume<'a, T>(
    chapters: impl IntoIterator<Item = (&'a Chapter, T)>,
) -> Vec<VolumeGroup<'a, T>> {
    let mut order: Vec<String> = Vec::new();
    let mut by_volume: std::collections::HashMap<String, Vec<(&Chapter, T)>> =
        std::collections::HashMap::new();

    for (chapter, with) in chapters {
        let volume_title = chapter
            .relative_path
            .parent()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".to_string());
        if !by_volume.contains_key(&volume_title) {
            order.push(volume_title.clone());
        }
        by_volume
            .entry(volume_title)
            .or_default()
            .push((chapter, with));
    }

    order
        .into_iter()
        .map(|title| {
            let chapters = by_volume.remove(&title).expect("just inserted above");
            VolumeGroup { title, chapters }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebook::Page;
    use std::path::PathBuf;

    fn chapter(path: &str, pages: usize) -> Chapter {
        Chapter {
            relative_path: PathBuf::from(path),
            title: path.rsplit('/').next().unwrap_or_default().to_string(),
            pages: (0..pages)
                .map(|_| Page {
                    extension: "png".to_string(),
                    ..Default::default()
                })
                .collect(),
        }
    }

    #[test]
    fn a_chapter_is_called_by_its_folder() {
        assert_eq!(
            chapter_label(&chapter("Vol 1/c001 - One", 1), "Book"),
            "c001 - One"
        );
    }

    #[test]
    fn pages_lying_directly_in_the_book_are_called_by_the_book() {
        let loose = Chapter {
            relative_path: PathBuf::new(),
            title: "Untitled".to_string(),
            pages: chapter("x", 1).pages,
        };
        assert_eq!(chapter_label(&loose, "Book"), "Book");
    }

    #[test]
    fn a_bookmark_without_pages_keeps_the_name_it_was_given() {
        let bookmark = Chapter {
            relative_path: PathBuf::new(),
            title: "Prologue".to_string(),
            pages: Vec::new(),
        };
        assert_eq!(chapter_label(&bookmark, "Book"), "Prologue");
    }

    #[test]
    fn chapters_are_grouped_by_their_volume_in_the_order_the_volumes_appear() {
        let chapters = [
            chapter("v001 - Vol.01/c001", 1),
            chapter("v001 - Vol.01/c002", 1),
            chapter("v002 - Vol.02/c001", 1),
        ];
        let groups = group_by_volume(chapters.iter().zip(0..));

        let shape: Vec<(&str, Vec<usize>)> = groups
            .iter()
            .map(|group| {
                (
                    group.title.as_str(),
                    group.chapters.iter().map(|(_, with)| *with).collect(),
                )
            })
            .collect();
        assert_eq!(
            shape,
            vec![("v001 - Vol.01", vec![0, 1]), ("v002 - Vol.02", vec![2])]
        );
    }

    #[test]
    fn a_volume_that_comes_back_later_is_still_one_volume() {
        let chapters = [chapter("A/c1", 1), chapter("B/c1", 1), chapter("A/c2", 1)];
        let groups = group_by_volume(chapters.iter().zip(0..));

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].title, "A");
        assert_eq!(groups[0].chapters.len(), 2);
        assert_eq!(groups[1].title, "B");
    }

    #[test]
    fn a_chapter_with_no_volume_folder_goes_under_untitled() {
        let chapters = [chapter("c001", 1)];
        let groups = group_by_volume(chapters.iter().zip(0..));

        assert_eq!(groups[0].title, "Untitled");
    }
}
