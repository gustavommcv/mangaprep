//! The outline of a PDF: the bookmarks a reader lists beside the pages, each one a title that
//! jumps to a page, and under a volume the chapters of it.
//!
//! `printpdf` can add bookmarks, but only as one flat list sorted by page and then by name, which
//! puts a chapter before the volume that starts on the same page and has no second level. The
//! outline is written here instead, into the `lopdf` document that `printpdf` hands over before it
//! is saved (ADR 0026).

use lopdf::{Dictionary, Document, Object, ObjectId};

/// One bookmark: its title, the page it opens (counted from 0, as the pages were added), and the
/// bookmarks under it.
pub(super) struct Entry {
    pub(super) label: String,
    pub(super) page: usize,
    pub(super) children: Vec<Entry>,
}

/// Adds the outline to the document and has a reader open its list. Does nothing for no entries.
pub(super) fn add(doc: &mut Document, entries: &[Entry]) {
    if entries.is_empty() {
        return;
    }
    let pages: Vec<ObjectId> = doc.get_pages().values().copied().collect();
    let Some(last_page) = pages.len().checked_sub(1) else {
        return;
    };
    let root = doc.new_object_id();
    let (first, last) = add_level(doc, &pages, last_page, entries, root);

    let mut outlines = Dictionary::new();
    outlines.set("Type", Object::Name(b"Outlines".to_vec()));
    outlines.set("First", Object::Reference(first));
    outlines.set("Last", Object::Reference(last));
    // The bookmarks a reader shows now: the top ones, since a volume starts closed.
    outlines.set("Count", Object::Integer(entries.len() as i64));
    doc.set_object(root, outlines);

    if let Ok(catalog) = doc.catalog_mut() {
        catalog.set("Outlines", Object::Reference(root));
        catalog.set("PageMode", Object::Name(b"UseOutlines".to_vec()));
    }
}

/// Writes one level of siblings under `parent`, and returns its first and last. The siblings take
/// their object numbers before any is written, so each can name the ones next to it.
fn add_level(
    doc: &mut Document,
    pages: &[ObjectId],
    last_page: usize,
    entries: &[Entry],
    parent: ObjectId,
) -> (ObjectId, ObjectId) {
    let ids: Vec<ObjectId> = entries.iter().map(|_| doc.new_object_id()).collect();
    for (index, entry) in entries.iter().enumerate() {
        let mut item = Dictionary::new();
        item.set("Title", text(&entry.label));
        item.set("Parent", Object::Reference(parent));
        // The entries are made from the pages that were just added, so the page is there; the
        // last one is only what an impossible number would open, not a place to fail.
        let page = pages[entry.page.min(last_page)];
        item.set(
            "Dest",
            Object::Array(vec![
                Object::Reference(page),
                Object::Name(b"XYZ".to_vec()),
                Object::Null,
                Object::Null,
                Object::Null,
            ]),
        );
        if let Some(previous) = index.checked_sub(1) {
            item.set("Prev", Object::Reference(ids[previous]));
        }
        if let Some(next) = ids.get(index + 1) {
            item.set("Next", Object::Reference(*next));
        }
        if !entry.children.is_empty() {
            let (first, last) = add_level(doc, pages, last_page, &entry.children, ids[index]);
            item.set("First", Object::Reference(first));
            item.set("Last", Object::Reference(last));
            // A negative count is a bookmark that starts closed, with this many under it.
            item.set("Count", Object::Integer(-(descendants(entry) as i64)));
        }
        doc.set_object(ids[index], item);
    }
    (ids[0], ids[ids.len() - 1])
}

fn descendants(entry: &Entry) -> usize {
    entry
        .children
        .iter()
        .map(|child| 1 + descendants(child))
        .sum()
}

/// A title as a PDF text string: UTF-16 with its byte order mark, which every reader understands
/// whatever the language of the title.
fn text(label: &str) -> Object {
    let mut bytes = vec![0xFE, 0xFF];
    for unit in label.encode_utf16() {
        bytes.extend_from_slice(&unit.to_be_bytes());
    }
    Object::string_literal(bytes)
}
