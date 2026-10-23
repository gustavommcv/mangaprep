import unittest

from real_books import compare


def book(cover, title="Book"):
    return {"pages": [], "meta": {"title": [title], "cover": cover}, "contents": []}


class Metadata(unittest.TestCase):
    def test_a_cover_only_mangaprep_declares_is_a_difference(self):
        problems, _ = compare(book([]), book(["Images/cover.jpg"]), "epub")
        self.assertEqual(problems, ["cover: KCC [], mangaprep ['Images/cover.jpg']"])

    def test_a_webtoon_book_declares_a_cover_where_kccs_does_not(self):
        # TOC-4 of ADR 0019, kept on purpose.
        problems, _ = compare(book([]), book(["Images/c0001/p0001.jpg"]), "epub", webtoon=True)
        self.assertEqual(problems, [])

    def test_a_webtoon_book_still_compares_the_rest(self):
        problems, _ = compare(book([], "One"), book(["x.jpg"], "Two"), "epub", webtoon=True)
        self.assertEqual(problems, ["title: KCC ['One'], mangaprep ['Two']"])


if __name__ == "__main__":
    unittest.main()
