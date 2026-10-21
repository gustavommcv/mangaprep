import unittest

from real_pages import parse_window_defaults, sample, window_arguments


TABLE = """
        profiles = {
            "Kindle 11": {
                'PVOptions': True, 'ForceExpert': False, 'DefaultFormat': 0, 'DefaultUpscale': True, 'ForceColor': False, 'Label': 'K11',
            },
            "Kindle Scribe 3": {
                'PVOptions': True, 'ForceExpert': False, 'DefaultFormat': 4, 'DefaultUpscale': False, 'ForceColor': False, 'Label': 'KS3',
            },
            "Kobo Libra Colour": {
                'PVOptions': False, 'ForceExpert': False, 'DefaultFormat': 1, 'DefaultUpscale': True, 'ForceColor': True, 'Label': 'KoLC',
            },
        }
"""


class WindowDefaults(unittest.TestCase):
    def test_reads_upscaling_and_colour_for_each_device(self):
        self.assertEqual(
            parse_window_defaults(TABLE),
            {"K11": (True, False), "KS3": (False, False), "KoLC": (True, True)},
        )

    def test_a_table_it_cannot_read_stops_the_run(self):
        with self.assertRaises(SystemExit):
            parse_window_defaults("profiles = {}")

    def test_the_options_follow_the_device(self):
        defaults = parse_window_defaults(TABLE)
        self.assertEqual(window_arguments("K11", defaults),
                         (["-p", "K11", "-m", "-r", "2", "-f", "CBZ", "-u"],
                          ["--manga", "--splitter=both", "--format=cbz", "--upscale"]))
        self.assertEqual(window_arguments("KS3", defaults),
                         (["-p", "KS3", "-m", "-r", "2", "-f", "CBZ"],
                          ["--manga", "--splitter=both", "--format=cbz"]))
        self.assertEqual(window_arguments("KoLC", defaults)[1][-2:], ["--upscale", "--forcecolor"])


class Sample(unittest.TestCase):
    def test_a_book_smaller_than_the_sample_is_taken_whole(self):
        self.assertEqual(sample(["a", "b"], 60), ["a", "b"])
        self.assertEqual(sample(["a", "b"], 0), ["a", "b"])


if __name__ == "__main__":
    unittest.main()
