//! PDF output. One page per processed image, sized to exactly that image's
//! pixel dimensions (1 pixel = 1 point) — mirrors `buildPDF()`'s intent in
//! upstream `comic2ebook.py` (GPLv3 — reimplemented from documented
//! behavior, not copied; see `docs/adr/0007-gplv3-boundary-kcc-image-rs.md`).
//! There is no PDF *input* to support here (see [`super`] module docs), only
//! already-processed raster pages to place, so the document is written
//! directly with `lopdf` (ADR 0027):
//!
//! - A JPEG page is embedded as it is (`DCTDecode`), as KCC embeds it: it is
//!   neither decoded nor compressed a second time, so the PDF holds the same
//!   pixels as the CBZ or EPUB of the same book, at the same size.
//! - Any other page (a PNG) is decoded and its pixels stored losslessly
//!   (`FlateDecode`): a grayscale page as one channel, a color page (only
//!   there with `--forcecolor`) as three.
//! - Every image is stored at its full size: nothing is reduced to fit a
//!   budget, and each page names only its own image.

mod outline;

use super::contents::{chapter_label, group_by_volume};
use super::Chapter;
use crate::error::{Error, Result};
use lopdf::{dictionary, Document, Object, Stream};
use outline::{text, Entry};

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

    let mut doc = Document::with_version("1.7");
    let pages_id = doc.new_object_id();
    let mut kids = Vec::new();
    // Where each chapter starts: the number, from 0, of the first page it adds.
    let mut chapter_starts = Vec::with_capacity(chapters.len());

    for chapter in &chapters {
        chapter_starts.push((*chapter, kids.len()));
        for page in &chapter.pages {
            let image = page_image(&page.bytes)?;
            let (width, height) = (i64::from(image.width), i64::from(image.height));
            let image_id = doc.add_object(image.stream);
            // The image is drawn over the whole page: scaled from its unit square to the page's
            // size in points, which is its size in pixels.
            let mut contents = Stream::new(
                dictionary! {},
                format!("q\n{width} 0 0 {height} 0 0 cm\n/Page Do\nQ\n").into_bytes(),
            );
            contents.compress().map_err(pdf_error)?;
            let contents_id = doc.add_object(contents);
            let page_id = doc.add_object(dictionary! {
                "Type" => "Page",
                "Parent" => pages_id,
                "MediaBox" => vec![0.into(), 0.into(), width.into(), height.into()],
                "Resources" => dictionary! { "XObject" => dictionary! { "Page" => image_id } },
                "Contents" => contents_id,
            });
            kids.push(Object::Reference(page_id));
        }
    }

    let count = kids.len() as i64;
    doc.objects.insert(
        pages_id,
        Object::Dictionary(dictionary! { "Type" => "Pages", "Kids" => kids, "Count" => count }),
    );
    let catalog_id = doc.add_object(dictionary! { "Type" => "Catalog", "Pages" => pages_id });
    doc.trailer.set("Root", catalog_id);
    let info_id = doc.add_object(dictionary! {
        "Title" => text(&options.title),
        "Author" => text(&options.author),
    });
    doc.trailer.set("Info", info_id);
    outline::add(
        &mut doc,
        &outline_entries(chapter_starts, &options.title, options.nested_toc),
    );

    let mut bytes = Vec::new();
    doc.save_to(&mut bytes).map_err(pdf_error)?;
    Ok(bytes)
}

fn pdf_error(error: impl std::fmt::Display) -> Error {
    Error::Pdf(error.to_string())
}

/// A page's image as the PDF stores it, and its size in pixels.
struct PageImage {
    width: u32,
    height: u32,
    stream: Stream,
}

fn page_image(bytes: &[u8]) -> Result<PageImage> {
    if let Some(components) = jpeg_components(bytes) {
        // The header alone gives the size, checked against the same pixel limit as a decode.
        let (width, height) = crate::input::image_dimensions(bytes)?;
        let color_space = if components == 1 {
            "DeviceGray"
        } else {
            "DeviceRGB"
        };
        let mut dict = image_dictionary(width, height, color_space);
        dict.set("Filter", "DCTDecode");
        return Ok(PageImage {
            width,
            height,
            stream: Stream::new(dict, bytes.to_vec()).with_compression(false),
        });
    }
    let decoded = crate::input::decode_image(bytes)?;
    let (width, height) = (decoded.width(), decoded.height());
    let (color_space, pixels) = if decoded.color().has_color() {
        ("DeviceRGB", decoded.to_rgb8().into_raw())
    } else {
        ("DeviceGray", decoded.to_luma8().into_raw())
    };
    let mut stream = Stream::new(image_dictionary(width, height, color_space), pixels);
    stream.compress().map_err(pdf_error)?;
    Ok(PageImage {
        width,
        height,
        stream,
    })
}

fn image_dictionary(width: u32, height: u32, color_space: &str) -> lopdf::Dictionary {
    dictionary! {
        "Type" => "XObject",
        "Subtype" => "Image",
        "Width" => i64::from(width),
        "Height" => i64::from(height),
        "ColorSpace" => color_space,
        "BitsPerComponent" => 8,
    }
}

/// The number of components of a JPEG a PDF reader can show as it is: an 8-bit Huffman-coded
/// frame, baseline or progressive, of one component (gray) or three (color). `None` for anything
/// else — not a JPEG, CMYK, 12-bit, lossless or arithmetic-coded — which is then decoded and
/// stored as pixels instead.
fn jpeg_components(bytes: &[u8]) -> Option<u8> {
    if !bytes.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut at = 2;
    loop {
        if *bytes.get(at)? != 0xFF {
            return None;
        }
        let marker = *bytes.get(at + 1)?;
        if marker == 0xFF {
            // A fill byte before a marker.
            at += 1;
            continue;
        }
        match marker {
            0xC0..=0xC2 => {
                let precision = *bytes.get(at + 4)?;
                let components = *bytes.get(at + 9)?;
                return (precision == 8 && matches!(components, 1 | 3)).then_some(components);
            }
            0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF | 0xD9 | 0xDA => return None,
            _ => {
                let length = usize::from(u16::from_be_bytes([
                    *bytes.get(at + 2)?,
                    *bytes.get(at + 3)?,
                ]));
                at += 2 + length;
            }
        }
    }
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
    use lopdf::ObjectId;
    use std::path::PathBuf;

    fn encoded(image: image::DynamicImage, format: image::ImageFormat) -> Vec<u8> {
        let mut bytes = Vec::new();
        image
            .write_to(&mut std::io::Cursor::new(&mut bytes), format)
            .unwrap();
        bytes
    }

    fn tiny_png(w: u32, h: u32) -> Vec<u8> {
        let img = image::GrayImage::from_pixel(w, h, image::Luma([128]));
        encoded(
            image::DynamicImage::ImageLuma8(img),
            image::ImageFormat::Png,
        )
    }

    fn page_of(bytes: Vec<u8>, extension: &str) -> Page {
        Page {
            extension: extension.to_string(),
            bytes,
            ..Default::default()
        }
    }

    fn chapter_with_pages(sizes: &[(u32, u32)]) -> Chapter {
        Chapter {
            relative_path: PathBuf::from("c001"),
            title: "c001".to_string(),
            pages: sizes
                .iter()
                .map(|&(w, h)| page_of(tiny_png(w, h), "png"))
                .collect(),
        }
    }

    fn chapter_of(pages: Vec<Page>) -> Chapter {
        Chapter {
            relative_path: PathBuf::from("c001"),
            title: "c001".to_string(),
            pages,
        }
    }

    fn default_options() -> PdfOptions {
        PdfOptions {
            nested_toc: false,
            title: "Test Book".to_string(),
            author: "Test Author".to_string(),
        }
    }

    fn load(bytes: &[u8]) -> Document {
        Document::load_mem(bytes).expect("mangapress's own PDF output should parse back cleanly")
    }

    fn page_ids(doc: &Document) -> Vec<ObjectId> {
        doc.get_pages().into_values().collect()
    }

    fn media_box(doc: &Document, page: ObjectId) -> Vec<f32> {
        doc.get_dictionary(page)
            .unwrap()
            .get(b"MediaBox")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|n| n.as_float().unwrap())
            .collect()
    }

    /// The images a page names in its resources, by object.
    fn images_of(doc: &Document, page: ObjectId) -> Vec<ObjectId> {
        let resources = doc
            .get_dictionary(page)
            .unwrap()
            .get(b"Resources")
            .unwrap()
            .as_dict()
            .unwrap();
        resources
            .get(b"XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .iter()
            .map(|(_, image)| image.as_reference().unwrap())
            .collect()
    }

    fn image_stream(doc: &Document, page: ObjectId) -> &Stream {
        let images = images_of(doc, page);
        doc.get_object(images[0]).unwrap().as_stream().unwrap()
    }

    fn number(dict: &lopdf::Dictionary, key: &[u8]) -> i64 {
        dict.get(key).unwrap().as_i64().unwrap()
    }

    fn name<'a>(dict: &'a lopdf::Dictionary, key: &[u8]) -> &'a [u8] {
        dict.get(key).unwrap().as_name().unwrap()
    }

    /// A text string of the Info dictionary, read back from its UTF-16.
    fn info_text(doc: &Document, key: &[u8]) -> String {
        let info = doc
            .get_dictionary(doc.trailer.get(b"Info").unwrap().as_reference().unwrap())
            .unwrap();
        let Ok(Object::String(raw, _)) = info.get(key) else {
            panic!("no {}", String::from_utf8_lossy(key));
        };
        assert_eq!(&raw[..2], &[0xFE, 0xFF]);
        let units: Vec<u16> = raw[2..]
            .chunks(2)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
            .collect();
        String::from_utf16(&units).unwrap()
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
        assert_eq!(load(&bytes).get_pages().len(), 3);
    }

    #[test]
    fn each_page_is_sized_to_its_own_image_in_points() {
        let chapters = vec![chapter_with_pages(&[(300, 150), (80, 200)])];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();
        let doc = load(&bytes);
        let pages = page_ids(&doc);

        assert_eq!(media_box(&doc, pages[0]), vec![0.0, 0.0, 300.0, 150.0]);
        assert_eq!(media_box(&doc, pages[1]), vec![0.0, 0.0, 80.0, 200.0]);
    }

    #[test]
    fn a_jpeg_page_is_embedded_as_it_is() {
        let gray = encoded(
            image::DynamicImage::ImageLuma8(image::GrayImage::from_fn(40, 60, |x, y| {
                image::Luma([((x * 5 + y * 3) % 256) as u8])
            })),
            image::ImageFormat::Jpeg,
        );
        let color = encoded(
            image::DynamicImage::ImageRgb8(image::RgbImage::from_fn(50, 30, |x, y| {
                image::Rgb([(x * 4) as u8, (y * 7) as u8, 90])
            })),
            image::ImageFormat::Jpeg,
        );
        let chapter = chapter_of(vec![
            page_of(gray.clone(), "jpg"),
            page_of(color.clone(), "jpg"),
        ]);
        let bytes = build_pdf(&[chapter], &default_options()).unwrap();
        let doc = load(&bytes);
        let pages = page_ids(&doc);

        for (page, jpeg, size, space) in [
            (pages[0], &gray, (40, 60), b"DeviceGray".as_slice()),
            (pages[1], &color, (50, 30), b"DeviceRGB".as_slice()),
        ] {
            let stream = image_stream(&doc, page);
            assert_eq!(name(&stream.dict, b"Filter"), b"DCTDecode");
            assert_eq!(name(&stream.dict, b"ColorSpace"), space);
            assert_eq!(
                (
                    number(&stream.dict, b"Width"),
                    number(&stream.dict, b"Height")
                ),
                size
            );
            assert_eq!(&stream.content, jpeg, "the page's own JPEG, byte for byte");
        }
    }

    #[test]
    fn a_large_color_page_keeps_every_pixel() {
        // 1600 x 1400 RGB is 6.7 MB of pixels: more than the 2 MB a PDF library may reduce an
        // image to by default. Stored losslessly, it must come back exactly.
        let original = image::RgbImage::from_fn(1600, 1400, |x, y| {
            image::Rgb([(x % 251) as u8, (y % 241) as u8, ((x + y) % 239) as u8])
        });
        let png = encoded(
            image::DynamicImage::ImageRgb8(original.clone()),
            image::ImageFormat::Png,
        );
        let bytes =
            build_pdf(&[chapter_of(vec![page_of(png, "png")])], &default_options()).unwrap();
        let doc = load(&bytes);
        let stream = image_stream(&doc, page_ids(&doc)[0]);

        assert_eq!(name(&stream.dict, b"Filter"), b"FlateDecode");
        assert_eq!(name(&stream.dict, b"ColorSpace"), b"DeviceRGB");
        assert_eq!(
            (
                number(&stream.dict, b"Width"),
                number(&stream.dict, b"Height")
            ),
            (1600, 1400)
        );
        assert_eq!(stream.decompressed_content().unwrap(), original.into_raw());
    }

    #[test]
    fn a_gray_png_page_is_stored_as_one_channel() {
        let bytes = build_pdf(&[chapter_with_pages(&[(20, 10)])], &default_options()).unwrap();
        let doc = load(&bytes);
        let stream = image_stream(&doc, page_ids(&doc)[0]);

        assert_eq!(name(&stream.dict, b"ColorSpace"), b"DeviceGray");
        assert_eq!(stream.decompressed_content().unwrap(), vec![128; 200]);
    }

    #[test]
    fn each_page_names_only_its_own_image() {
        let chapters = vec![chapter_with_pages(&[(10, 10), (12, 12), (14, 14)])];
        let bytes = build_pdf(&chapters, &default_options()).unwrap();
        let doc = load(&bytes);
        let images: Vec<Vec<ObjectId>> = page_ids(&doc)
            .into_iter()
            .map(|page| images_of(&doc, page))
            .collect();

        assert!(images.iter().all(|names| names.len() == 1));
        assert_ne!(images[0], images[1]);
        assert_ne!(images[1], images[2]);
    }

    #[test]
    fn a_file_that_only_starts_like_a_jpeg_is_decoded_instead() {
        assert_eq!(jpeg_components(&[0xFF, 0xD8]), None);
        assert_eq!(jpeg_components(&[0xFF, 0xD8, 0xFF, 0xE0, 0x00]), None);
        assert_eq!(jpeg_components(b"\x89PNG\r\n"), None);
    }

    /// A chapter in a folder of its own, with that many one-pixel pages.
    fn chapter_at(path: &str, pages: usize) -> Chapter {
        Chapter {
            relative_path: PathBuf::from(path),
            title: path.rsplit('/').next().unwrap_or_default().to_string(),
            pages: (0..pages).map(|_| page_of(tiny_png(4, 4), "png")).collect(),
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
            author: "藤本タツキ".to_string(),
        };
        let bytes = build_pdf(&chapters, &options).unwrap();
        let doc = load(&bytes);
        assert_eq!(info_text(&doc, b"Title"), "Chainsaw Man Vol. 01");
        assert_eq!(info_text(&doc, b"Author"), "藤本タツキ");
    }
}
