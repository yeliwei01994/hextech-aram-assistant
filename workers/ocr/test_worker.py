import json
import unittest
from pathlib import Path
from unittest.mock import patch

from PIL import Image, ImageDraw

import worker


class ScreenDetectionTests(unittest.TestCase):
    def test_shop_requires_more_than_one_corner(self):
        frame = Image.new("RGB", (1200, 675), (22, 37, 49))
        draw = ImageDraw.Draw(frame)
        gold = (167, 122, 70)
        draw.line((120, 50, 800, 50, 800, 530), fill=gold, width=3)
        self.assertFalse(worker.shop_is_visible(frame))
        draw.rectangle((120, 50, 800, 530), outline=gold, width=3)
        self.assertTrue(worker.shop_is_visible(frame))

    def test_real_shop_screenshot(self):
        path = Path(__file__).resolve().parents[2] / "docs/screenshots/shop-advice-overlay.png"
        with Image.open(path) as frame:
            self.assertTrue(worker.shop_is_visible(frame.convert("RGB")))

    def test_augment_choice_takes_priority_over_shop(self):
        layout = json.loads(Path(__file__).with_name("card-layout.json").read_text())
        rect = {"left": 0, "top": 0, "width": 1280, "height": 720}
        frame = Image.new("RGB", (1280, 720))
        cards = [{"slot": slot, "matched": True} for slot in range(1, 4)]
        with (
            patch.object(worker, "active_game_client_rect", return_value=rect),
            patch.object(worker, "capture_rgb", return_value=frame) as capture,
            patch.object(worker, "shop_is_visible", return_value=True) as shop,
            patch.object(worker, "detect_augment_layout", return_value=(layout, cards)),
        ):
            result = worker.process_request({"action": "probe_augment_screen"}, layout, [None], {})
        self.assertTrue(result["augment_choice"])
        self.assertFalse(result["shop_visible"])
        capture.assert_called_once()
        shop.assert_not_called()

    def test_dim_but_readable_titles_have_distinct_signatures(self):
        first = Image.new("RGB", (200, 24))
        second = first.copy()
        ImageDraw.Draw(first).text((85, 4), "First", fill=(130, 130, 130))
        ImageDraw.Draw(second).text((85, 4), "Other", fill=(130, 130, 130))
        self.assertNotEqual(worker.title_signature(first), worker.title_signature(second))

    def test_reroll_only_recognizes_the_changed_slot(self):
        layout = json.loads(Path(__file__).with_name("card-layout.json").read_text())
        rect = {"left": 0, "top": 0, "width": 1280, "height": 720}
        first = Image.new("RGB", (1280, 720))
        second = first.copy()
        for index, card in enumerate(layout["cards"]):
            title_x = round(1280 * (card["x"] + card["width"] * .5))
            title_y = round(720 * (card["y"] + card["height"] * .46))
            ImageDraw.Draw(first).text((title_x, title_y), "First", fill=(130, 130, 130))
            ImageDraw.Draw(second).text(
                (title_x, title_y), "Other" if index == 1 else "First", fill=(130, 130, 130)
            )
        metrics = [{"matched": True} for _ in layout["cards"]]
        request = {"action": "recognize_augment_choice", "language": "ch"}
        cache = {}
        engine = [object()]
        with (
            patch.object(worker, "active_game_client_rect", return_value=rect),
            patch.object(worker, "containing_monitor", return_value=rect),
            patch.object(worker, "detect_augment_layout", return_value=(layout, metrics)),
            patch.object(worker, "capture_rgb", side_effect=[first, second]),
            patch.object(worker, "recognize_cards", side_effect=[
                [("First", .99)] * 3, [("Other", .99)],
            ]) as recognize,
        ):
            worker.process_request(request, layout, engine, cache)
            result = worker.process_request(request, layout, engine, cache)
        self.assertEqual([card["raw_text"] for card in result["cards"]], ["First", "Other", "First"])
        self.assertEqual(len(recognize.call_args.args[1]), 1)


if __name__ == "__main__":
    unittest.main()
