//! PDF output. One page per processed image, sized to exactly that image's
//! pixel dimensions (1 pixel = 1 point) — mirrors `buildPDF()`'s intent in
//! upstream `comic2ebook.py` (GPLv3 — reimplemented from documented
//! behavior, not copied; see `docs/adr/0007-gplv3-boundary-kcc-image-rs.md`),
//! but via `printpdf` instead of PyMuPDF, since there is no PDF *input* to
//! support here (see [`super`] module docs) — only building output pages
//! from already-processed raster images.
//!
//! `printpdf`'s `RawImage::from_dynamic_image()` accepts an already-decoded
//! image directly, so each page only needs decoding once (there is no
//! bytes -> `RawImage` -> re-encode round trip). A `DynamicImage` is always
//! normalized to `Luma8` for grayscale pages or `Rgb8` for color pages before
//! that call. Color output is supported; normalization avoids depending on
//! JPEG/PNG round-tripping through the `image` crate always preserving the
//! original color type.
//!
//! Placing an image at exactly 1 pixel = 1 point (rather than printpdf's
//! own default of 1 pixel = 1/300 inch) needs `XObjectTransform`'s `dpi`
//! field explicitly set to `72.0`: `into_pt(dpi)` computes `px * 72.0/dpi`,
//! which only reduces to `px` (matching the page's own point-sized
//! dimensions exactly) when `dpi == 72.0` — confirmed by reading
//! `XObjectTransform::get_ctms()` in printpdf's source, not assumed from
//! its (points-per-inch-flavored) documentation alone.

mod outline;

use super::contents::{chapter_label, group_by_volume};
use super::Chapter;
use crate::error::{Error, Result};
use image::DynamicImage;
use outline::Entry;
use printpdf::{Mm, Op, PdfDocument, PdfPage, PdfSaveOptions, Pt, RawImage, XObjectTransform};

/// Resolved title/author to embed in the PDF's own Info dictionary — mirrors
/// [`super::epub::EpubOptions`]'s title/author fields (see
/// [`crate::metadata::resolve`], which produces both from the CLI and any
/// `ComicInfo.xml`).
pub struct PdfOptions {
    pub title: String,
    pub author: String,
    /// Expect each [`Chapter`]'s `relative_path` to carry one more directory level above it (a
    /// volume), and give the outline two levels: a bookmark for each volume with its chapters
    /// under it, instead of one flat list of chapters. The same shape as the EPUB's
    /// `nested_toc`, from the same producer (Mangabind's `-combine`).
    pub nested_toc: bool,
}

pub fn build_pdf(chapters: &[Chapter], options: &PdfOptions) -> Result<Vec<u8>> {
    let chapters: Vec<&Chapter> = chapters.iter().filter(|c| !c.pages.is_empty()).collect();
    if chapters.is_empty() {
        return Err(Error::EmptyBook);
    }

    let mut doc = PdfDocument::new(&options.title);
    doc.metadata.info.document_title = options.title.clone();
    doc.metadata.info.author = options.author.clone();
    let mut pdf_pages = Vec::new();
    // Where each chapter starts: the number, from 0, of the first page it adds.
    let mut chapter_starts = Vec::with_capacity(chapters.len());

    for chapter in &chapters {
        chapter_starts.push((*chapter, pdf_pages.len()));
        for page in &chapter.pages {
            let decoded = crate::input::decode_image(&page.bytes)?;
            let (width_px, height_px) = (decoded.width(), decoded.height());
            // Grayscale pages as single-channel images; a color page (only
            // there with `--forcecolor`) stays RGB.
            let normalized = if decoded.color().has_color() {
                DynamicImage::ImageRgb8(decoded.to_rgb8())
            } else {
                DynamicImage::ImageLuma8(decoded.to_luma8())
            };

            let raw_image = RawImage::from_dynamic_image(normalized).map_err(Error::Pdf)?;
            let xobject_id = doc.add_image(&raw_image);

            let ops = vec![Op::UseXobject {
                id: xobject_id,
                transform: XObjectTransform {
                    dpi: Some(72.0),
                    ..Default::default()
                },
            }];
            pdf_pages.push(PdfPage::new(
                Mm::from(Pt(width_px as f32)),
                Mm::from(Pt(height_px as f32)),
                ops,
            ));
        }
    }

    // `printpdf` saves a document it can only give bookmarks as one flat list, so the outline is
    // written into the document it hands over, and the saving it does is done here, as it does it.
    let save_options = PdfSaveOptions::default();
    let mut warnings = Vec::new();
    let mut document = doc
        .with_pages(pdf_pages)
        .to_lopdf_document(&save_options, &mut warnings);
    outline::add(
        &mut document,
        &outline_entries(chapter_starts, &options.title, options.nested_toc),
    );
    if save_options.optimize {
        document.compress();
    }
    let mut bytes = Vec::new();
    document
        .save_to(&mut bytes)
        .map_err(|error| Error::Pdf(error.to_string()))?;
    Ok(bytes)
}

/// The bookmarks of the book: one for each chapter, named as the EPUB's contents name it, or, for
/// a book of volumes, one for each volume with its chapters under it.
fn outline_entries(
    chapter_starts: Vec<(&Chapter, usize)>,
    book_title: &str,
    nested: bool,
) -> Vec<Entry> {
    let entry_of = |(chapter, page): (&Chapter, usize)| Entry {
        label: chapter_label(chapter, book_title).to_string(),
        page,
        children: Vec::new(),
    };
    if !nested {
        return chapter_starts.into_iter().map(entry_of).collect();
    }
    group_by_volume(chapter_starts)
        .into_iter()
        .map(|volume| Entry {
            label: volume.title,
            // The volume opens where its first chapter does.
            page: volume.chapters[0].1,
            children: volume.chapters.into_iter().map(entry_of).collect(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ebook::Page;
    use std::path::PathBuf;

    fn tiny_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::GrayImage::from_pixel(w, h, image::Luma([128]));
        let mut bytes = Vec::new();
        image::DynamicImage::ImageLuma8(img)
            .write_to(
                &mut std::io::Cursor::new(&mut bytes),
                image::ImageFormat::Png,
            )
            .unwrap();
        bytes
    }

    fn chapter_with_pages(sizes: &[(u32, u32)]) -> Chapter {
        Chapter {
            relative_path: PathBuf::from("c001"),
            title: "c001".to_string(),
            pages: sizes
                .iter()
                .map(|&(w, h)| Page {
                    extension: "png".to_string(),
                    bytes: tiny_png(w, h),
                    ..Default::default()
                })
                .collect(),
        }
    }

    fn parse_back(bytes: &[u8]) -> PdfDocument {
        let mut warnings = Vec::new();
        PdfDocument::parse(bytes, &printpdf::PdfParseOptions::default(), &mut warnings)
            .expect("mangapress's own PDF output should parse back cleanly")
    }

    fn default_options() -> PdfOptions {
        PdfOptions {
            nested_toc: false,
            title: "Test Book".to_string(),
            author: "Test Author".to_string(),
        }
    }

    #[test]
    fn empty_book_is_rejected() {
        assert!(matches!(
            build_pdf(&[], &default_options()),
            Err(Error::EmptyBook)
        ));
    }

    #[test]
    fn a_library_supplied_oversized_page_is_not_decoded_by_the_pdf_builder() {
        let mut chapter = chapter_with_pages(&[(3, 7)]);
        chapter.pages[0].bytes = crate::test_support::oversized_bmp();
        assert!(matches!(
            build_pdf(&[chapter], &default_options()),
            Err(Error::ImageTooLarge { .. })
        ));
    }

    #[test]
    fn starts_with_the_pdf_header() {
        let chapters = vec![chapter_with_pages(&[(100, 150)])];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();
        assert!(bytes.starts_with(b"%PDF"), "output should be a real PDF");
    }

    #[test]
    fn page_count_matches_total_pages_across_chapters() {
        let chapters = vec![
            chapter_with_pages(&[(100, 100), (100, 100)]),
            chapter_with_pages(&[(100, 100)]),
        ];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();
        let parsed = parse_back(&bytes);
        assert_eq!(parsed.pages.len(), 3);
    }

    #[test]
    fn each_page_is_sized_to_its_own_image_in_points() {
        // 72 DPI is chosen specifically so 1 pixel == 1 point -- confirmed
        // by reading XObjectTransform::get_ctms() rather than assumed from
        // documentation (see module docs).
        let chapters = vec![chapter_with_pages(&[(300, 150), (80, 200)])];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();
        let parsed = parse_back(&bytes);

        assert_eq!(parsed.pages.len(), 2);
        assert!((parsed.pages[0].media_box.width.0 - 300.0).abs() < 0.5);
        assert!((parsed.pages[0].media_box.height.0 - 150.0).abs() < 0.5);
        assert!((parsed.pages[1].media_box.width.0 - 80.0).abs() < 0.5);
        assert!((parsed.pages[1].media_box.height.0 - 200.0).abs() < 0.5);
    }

    /// A chapter in a folder of its own, with that many one-pixel pages.
    fn chapter_at(path: &str, pages: usize) -> Chapter {
        Chapter {
            relative_path: PathBuf::from(path),
            title: path.rsplit('/').next().unwrap_or_default().to_string(),
            pages: (0..pages)
                .map(|_| Page {
                    extension: "png".to_string(),
                    bytes: tiny_png(4, 4),
                    ..Default::default()
                })
                .collect(),
        }
    }

    /// A bookmark as a reader lists it: its title, the page it opens (from 1) and the bookmarks
    /// under it, read back from the saved file.
    #[derive(Debug, PartialEq)]
    struct Bookmark {
        title: String,
        page: u32,
        children: Vec<Bookmark>,
    }

    fn read_outline(bytes: &[u8]) -> Vec<Bookmark> {
        use lopdf::{Document, Object, ObjectId};

        fn title(item: &lopdf::Dictionary) -> String {
            let Ok(Object::String(raw, _)) = item.get(b"Title") else {
                panic!("a bookmark has no title");
            };
            assert_eq!(
                &raw[..2],
                &[0xFE, 0xFF],
                "a title starts with its byte order mark"
            );
            let units: Vec<u16> = raw[2..]
                .chunks(2)
                .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
                .collect();
            String::from_utf16(&units).unwrap()
        }

        fn level(
            doc: &Document,
            pages: &std::collections::BTreeMap<ObjectId, u32>,
            first: Option<&Object>,
        ) -> Vec<Bookmark> {
            let mut items = Vec::new();
            let mut next = first.and_then(|object| object.as_reference().ok());
            while let Some(id) = next {
                let item = doc.get_dictionary(id).unwrap();
                let Ok(Object::Array(dest)) = item.get(b"Dest") else {
                    panic!("a bookmark opens no page");
                };
                let page = pages[&dest[0].as_reference().unwrap()];
                items.push(Bookmark {
                    title: title(item),
                    page,
                    children: level(doc, pages, item.get(b"First").ok()),
                });
                next = item.get(b"Next").ok().and_then(|o| o.as_reference().ok());
            }
            items
        }

        let doc = Document::load_mem(bytes).expect("the PDF opens");
        let pages: std::collections::BTreeMap<ObjectId, u32> =
            doc.get_pages().into_iter().map(|(n, id)| (id, n)).collect();
        let catalog = doc.catalog().unwrap();
        let Ok(outlines) = catalog.get(b"Outlines") else {
            return Vec::new();
        };
        let outlines = doc
            .get_dictionary(outlines.as_reference().unwrap())
            .unwrap();
        level(&doc, &pages, outlines.get(b"First").ok())
    }

    fn leaf(title: &str, page: u32) -> Bookmark {
        Bookmark {
            title: title.to_string(),
            page,
            children: Vec::new(),
        }
    }

    #[test]
    fn each_chapter_is_a_bookmark_that_opens_its_first_page() {
        let chapters = vec![
            chapter_at("c001 - One", 2),
            chapter_at("c002 - Two", 1),
            chapter_at("c003 - Three", 3),
        ];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();

        assert_eq!(
            read_outline(&bytes),
            vec![
                leaf("c001 - One", 1),
                leaf("c002 - Two", 3),
                leaf("c003 - Three", 4)
            ]
        );
    }

    #[test]
    fn a_reader_opens_the_list_of_bookmarks_with_the_document() {
        let bytes = build_pdf(&[chapter_at("c001", 1)], &default_options()).unwrap();
        let doc = lopdf::Document::load_mem(&bytes).unwrap();

        assert_eq!(
            doc.catalog()
                .unwrap()
                .get(b"PageMode")
                .unwrap()
                .as_name()
                .unwrap(),
            b"UseOutlines"
        );
    }

    #[test]
    fn the_title_of_a_bookmark_is_kept_in_any_language() {
        let bytes = build_pdf(
            &[
                chapter_at("第1話 犬とチェンソー", 1),
                chapter_at("Capítulo 2", 1),
            ],
            &default_options(),
        )
        .unwrap();

        assert_eq!(
            read_outline(&bytes),
            vec![leaf("第1話 犬とチェンソー", 1), leaf("Capítulo 2", 2)]
        );
    }

    #[test]
    fn pages_lying_directly_in_the_book_are_a_bookmark_named_by_the_book() {
        let loose = Chapter {
            relative_path: PathBuf::new(),
            title: "Untitled".to_string(),
            pages: chapter_at("x", 2).pages,
        };
        let bytes = build_pdf(&[loose], &default_options()).unwrap();

        assert_eq!(read_outline(&bytes), vec![leaf("Test Book", 1)]);
    }

    #[test]
    fn a_chapter_with_no_pages_has_no_bookmark_and_the_others_keep_their_pages() {
        let chapters = vec![
            chapter_at("c001", 1),
            chapter_at("c002", 0),
            chapter_at("c003", 2),
        ];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();

        assert_eq!(read_outline(&bytes), vec![leaf("c001", 1), leaf("c003", 2)]);
    }

    #[test]
    fn a_book_of_volumes_has_a_bookmark_for_each_volume_with_its_chapters_under_it() {
        let chapters = vec![
            chapter_at("v001 - Vol.01/c001", 2),
            chapter_at("v001 - Vol.01/c002", 1),
            chapter_at("v002 - Vol.02/c001", 3),
        ];
        let options = PdfOptions {
            nested_toc: true,
            ..default_options()
        };
        let bytes = build_pdf(&chapters, &options).unwrap();

        assert_eq!(
            read_outline(&bytes),
            vec![
                Bookmark {
                    title: "v001 - Vol.01".to_string(),
                    page: 1,
                    children: vec![leaf("c001", 1), leaf("c002", 3)],
                },
                Bookmark {
                    title: "v002 - Vol.02".to_string(),
                    page: 4,
                    children: vec![leaf("c001", 4)],
                },
            ]
        );
    }

    #[test]
    fn a_volume_comes_before_its_first_chapter_though_both_open_the_same_page() {
        // A flat list sorted by page and name would put "c001" before "v001 - Vol.01".
        let chapters = vec![chapter_at("v001 - Vol.01/c001", 1)];
        let options = PdfOptions {
            nested_toc: true,
            ..default_options()
        };
        let bytes = build_pdf(&chapters, &options).unwrap();
        let outline = read_outline(&bytes);

        assert_eq!(outline[0].title, "v001 - Vol.01");
        assert_eq!(outline[0].children[0].title, "c001");
    }

    #[test]
    fn resolved_title_and_author_are_embedded_in_the_pdf_info_dictionary() {
        let chapters = vec![chapter_with_pages(&[(100, 100)])];
        let options = PdfOptions {
            nested_toc: false,
            title: "Chainsaw Man Vol. 01".to_string(),
            author: "Tatsuki Fujimoto".to_string(),
        };
        let bytes = build_pdf(&chapters, &options).unwrap();
        let parsed = parse_back(&bytes);
        assert_eq!(parsed.metadata.info.document_title, "Chainsaw Man Vol. 01");
        assert_eq!(parsed.metadata.info.author, "Tatsuki Fujimoto");
    }
}
