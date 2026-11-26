from __future__ import annotations

import argparse
import contextlib
import ctypes
import hashlib
import json
import os
import sys
import time
from ctypes import wintypes
from pathlib import Path
from typing import Any

from mss import mss
from PIL import Image

MIN_BRIGHT_BORDER_RATIO = 0.22
MIN_DARK_CENTER_RATIO = 0.45


def set_per_monitor_dpi_awareness() -> None:
    if os.name != "nt":
        return
    try:
        ctypes.windll.user32.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4))
    except (AttributeError, OSError):
        try:
            ctypes.windll.shcore.SetProcessDpiAwareness(2)
        except (AttributeError, OSError):
            pass


def active_game_client_rect() -> dict[str, int]:
    if os.name != "nt":
        raise RuntimeError("当前 OCR 捕获仅支持 Windows 游戏客户端。")

    user32 = ctypes.WinDLL("user32", use_last_error=True)
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    user32.GetForegroundWindow.restype = wintypes.HWND
    user32.GetWindowThreadProcessId.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.DWORD)]
    kernel32.OpenProcess.argtypes = [wintypes.DWORD, wintypes.BOOL, wintypes.DWORD]
    kernel32.OpenProcess.restype = wintypes.HANDLE
    kernel32.QueryFullProcessImageNameW.argtypes = [
        wintypes.HANDLE,
        wintypes.DWORD,
        wintypes.LPWSTR,
        ctypes.POINTER(wintypes.DWORD),
    ]
    kernel32.QueryFullProcessImageNameW.restype = wintypes.BOOL
    kernel32.CloseHandle.argtypes = [wintypes.HANDLE]
    user32.GetClientRect.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.RECT)]
    user32.GetClientRect.restype = wintypes.BOOL
    user32.ClientToScreen.argtypes = [wintypes.HWND, ctypes.POINTER(wintypes.POINT)]
    user32.ClientToScreen.restype = wintypes.BOOL
    hwnd = user32.GetForegroundWindow()
    if not hwnd:
        raise RuntimeError("没有找到前台窗口，请切回游戏中的海克斯选择界面。")

    process_id = wintypes.DWORD()
    user32.GetWindowThreadProcessId(hwnd, ctypes.byref(process_id))
    handle = kernel32.OpenProcess(0x1000, False, process_id.value)
    if not handle:
        raise RuntimeError("无法确认前台程序，请切回游戏中的海克斯选择界面。")
    try:
        image_path = ctypes.create_unicode_buffer(32768)
        image_path_size = wintypes.DWORD(len(image_path))
        if not kernel32.QueryFullProcessImageNameW(
            handle, 0, image_path, ctypes.byref(image_path_size)
        ):
            raise RuntimeError("无法确认前台程序，请切回游戏中的海克斯选择界面。")
        process_name = Path(image_path.value).name.casefold()
    finally:
        kernel32.CloseHandle(handle)

    if process_name != "league of legends.exe":
        raise RuntimeError("请切回《英雄联盟》对局中的海克斯选择界面再识别。")

    rect = wintypes.RECT()
    if not user32.GetClientRect(hwnd, ctypes.byref(rect)):
        raise RuntimeError("无法读取游戏画面区域。")
    origin = wintypes.POINT(rect.left, rect.top)
    if not user32.ClientToScreen(hwnd, ctypes.byref(origin)):
        raise RuntimeError("无法定位游戏画面区域。")
    width = rect.right - rect.left
    height = rect.bottom - rect.top
    if width < 640 or height < 480:
        raise RuntimeError("游戏窗口尺寸过小，无法识别海克斯卡片。")
    return {"left": origin.x, "top": origin.y, "width": width, "height": height, "window_handle": int(hwnd)}


def containing_monitor(rect: dict[str, int]) -> dict[str, int]:
    center_x = rect["left"] + rect["width"] // 2
    center_y = rect["top"] + rect["height"] // 2
    with mss() as capture:
        monitors = capture.monitors[1:]
    if not monitors:
        raise RuntimeError("没有检测到可用显示器。")
    monitor = next(
        (
            item
            for item in monitors
            if item["left"] <= center_x < item["left"] + item["width"]
            and item["top"] <= center_y < item["top"] + item["height"]
        ),
        max(
            monitors,
            key=lambda item: max(
                0,
                min(rect["left"] + rect["width"], item["left"] + item["width"])
                - max(rect["left"], item["left"]),
            )
            * max(
                0,
                min(rect["top"] + rect["height"], item["top"] + item["height"])
                - max(rect["top"], item["top"]),
            ),
        ),
    )
    return {key: int(monitor[key]) for key in ("left", "top", "width", "height")}


def reference_rect(rect: dict[str, int]) -> dict[str, int]:
    """Center the 16:9 card layout inside the captured game client."""
    width = min(rect["width"], round(rect["height"] * 16 / 9))
    height = min(rect["height"], round(width * 9 / 16))
    return {"left": rect["left"] + (rect["width"] - width) // 2,
            "top": rect["top"] + (rect["height"] - height) // 2,
            "width": width, "height": height}


def capture_rgb(rect: dict[str, int]) -> Image.Image:
    with mss() as capture:
        screenshot = capture.grab(rect)
    return Image.frombytes("RGB", screenshot.size, screenshot.bgra, "raw", "BGRX")


def crop_region(frame: Image.Image, rect: dict[str, int], region: dict[str, int]) -> Image.Image:
    x = region["left"] - rect["left"]
    y = region["top"] - rect["top"]
    return frame.crop((x, y, x + region["width"], y + region["height"]))


def crop_title(
    rect: dict[str, int],
    card: dict[str, Any],
    title: dict[str, float],
    upscale: int,
    frame: Image.Image,
) -> tuple[Image.Image, str]:
    card_x = rect["left"] + round(rect["width"] * float(card["x"]))
    card_y = rect["top"] + round(rect["height"] * float(card["y"]))
    card_width = round(rect["width"] * float(card["width"]))
    card_height = round(rect["height"] * float(card["height"]))
    crop = {
        "left": card_x + round(card_width * float(title["x"])),
        "top": card_y + round(card_height * float(title["y"])),
        "width": round(card_width * float(title["width"])),
        "height": round(card_height * float(title["height"])),
    }
    if crop["width"] < 24 or crop["height"] < 16:
        raise RuntimeError("当前分辨率下标题区域太小，无法进行 OCR。")
    image = crop_region(frame, rect, crop)
    signature = title_signature(image)
    if upscale > 1:
        image = image.resize(
            (image.width * upscale, image.height * upscale), Image.Resampling.LANCZOS
        )
    return image, signature


def title_signature(image: Image.Image) -> str:
    # Titles are static while the card art and border animate. A small binary
    # mask keeps the signature focused on the text and cheap to compare.
    # Threshold before downsampling so thin or dim strokes do not disappear.
    strokes = image.convert("L").point(lambda pixel: 255 if pixel >= 115 else 0)
    sample = strokes.resize((64, 16), Image.Resampling.BOX)
    mask = sample.point(lambda pixel: 255 if pixel >= 64 else 0).tobytes()
    return hashlib.blake2s(mask, digest_size=8).hexdigest()


def probe_augment_cards(rect: dict[str, int], layout: dict[str, Any], frame: Image.Image) -> list[dict[str, Any]]:
    results = []
    for card in layout["cards"]:
        region = {
            "left": rect["left"] + round(rect["width"] * float(card["x"])),
            "top": rect["top"] + round(rect["height"] * float(card["y"])),
            "width": round(rect["width"] * float(card["width"])),
            "height": round(rect["height"] * float(card["height"])),
        }
        image = crop_region(frame, rect, region)
        title_left = round(image.width * float(layout["title_crop"]["x"]))
        title_top = round(image.height * float(layout["title_crop"]["y"]))
        title_right = title_left + round(image.width * float(layout["title_crop"]["width"]))
        title_bottom = title_top + round(image.height * float(layout["title_crop"]["height"]))
        signature = title_signature(image.crop((
            title_left, title_top, title_right, title_bottom,
        )))
        image = image.resize((32, 48), Image.Resampling.BILINEAR)
        pixels = image.load()

        # The card's luminous frame sits a few pixels inside its dark outer edge.
        border_pixels = []
        border_pixels.extend(pixels[x, y] for y in range(0, 2) for x in range(7, 25))
        border_pixels.extend(pixels[x, y] for y in range(46, 48) for x in range(7, 25))
        border_pixels.extend(pixels[x, y] for x in range(0, 2) for y in range(8, 40))
        border_pixels.extend(pixels[x, y] for x in range(30, 32) for y in range(8, 40))
        bright_border_ratio = sum(
            min(red, green, blue) > 130 and (red + green + blue) / 3 > 165
            for red, green, blue in border_pixels
        ) / len(border_pixels)

        center_pixels = [
            pixels[x, y]
            for y in range(7, 42)
            for x in range(5, 27)
        ]
        dark_center_ratio = sum(
            max(red, green, blue) < 105 for red, green, blue in center_pixels
        ) / len(center_pixels)
        results.append({
            "slot": int(card["slot"]),
            "signature": signature,
            "bright_border_ratio": round(bright_border_ratio, 3),
            "dark_center_ratio": round(dark_center_ratio, 3),
            "matched": (
                bright_border_ratio >= MIN_BRIGHT_BORDER_RATIO
                and dark_center_ratio >= MIN_DARK_CENTER_RATIO
            ),
        })
    return results


def locate_card_row(image: Image.Image, count: int) -> list[dict[str, Any]]:
    import cv2
    import numpy as np

    frame = np.asarray(image)
    width = 960
    height = round(width * image.height / image.width)
    frame = cv2.resize(frame, (width, height), interpolation=cv2.INTER_AREA)
    # Saturated frames can have a low red or green channel even when
    # their outline is bright. Find outlines by channel intensity, then
    # validate the card geometry and dark interiors for the complete row.
    intensity = frame.max(axis=2)
    # The silver three-card frame can fade below 120 near its lower edge.
    thresholds = (150, 120, 105) if count == 3 else (150, 120)
    for minimum in thresholds:
        mask = cv2.inRange(intensity, minimum, 255)
        mask = cv2.morphologyEx(mask, cv2.MORPH_CLOSE, np.ones((5, 5), np.uint8))
        if cards := find_card_row(frame, mask, count):
            return cards
    return []


def find_card_row(frame, mask, count):
    import cv2
    import numpy as np
    from itertools import combinations

    height, width = frame.shape[:2]
    contours, _ = cv2.findContours(mask, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    candidates = []
    for contour in contours:
        x, y, w, h = cv2.boundingRect(contour)
        if not (.12 <= w / width <= .24 and .34 <= h / height <= .66
                and .08 <= y / height <= .37 and cv2.contourArea(contour) >= w * h * .55):
            continue
        center = frame[y+h//5:y+4*h//5, x+w//5:x+4*w//5, :3]
        if np.mean(np.max(center, axis=2) < 105) >= .45:
            candidates.append((x, y, w, h))
    if not count <= len(candidates) <= 12:
        return []
    for group in combinations(sorted(candidates), count):
        xs, ys, ws, hs = zip(*group)
        steps = [b - a for a, b in zip(xs, xs[1:])]
        if (max(ys) - min(ys) > height * .035 or max(hs) - min(hs) > height * .08
                or max(ws) - min(ws) > width * .045 or min(steps) < width * .15
                or max(steps) - min(steps) > width * .035
                or (count == 3 and abs((xs[0] + xs[-1] + ws[-1]) / 2 - width / 2) > width * .05)
                or any(x + w > right for x, w, right in zip(xs, ws, xs[1:]))):
            continue
        return [{"slot": slot, "x": x / width, "y": y / height,
                 "width": w / width, "height": h / height}
                for slot, (x, y, w, h) in enumerate(group, 1)]
    return []


def detect_augment_layout(rect: dict[str, int], layout: dict[str, Any], frame: Image.Image):
    three = probe_augment_cards(rect, layout, frame)
    four_cards = layout.get("four_cards")
    if four_cards:
        four_layout = {**layout, "cards": four_cards}
        four = probe_augment_cards(rect, four_layout, frame)
        four_matches = sum(card["matched"] for card in four)
        three_matches = sum(card["matched"] for card in three)
        fixed_four_matches = four_matches == 4 or (four_matches >= 3 and four_matches > three_matches)
        if (fixed_four_matches or three_matches < 2) and (located := locate_card_row(frame, 4)):
            located_layout = {**layout, "cards": located}
            metrics = probe_augment_cards(rect, located_layout, frame)
            # The full row passed the geometry and interior checks. Keep the
            # title signatures, even if the fixed edge samples miss the frame.
            return located_layout, [{**card, "matched": True} for card in metrics]
        if fixed_four_matches:
            return four_layout, four
    if sum(card["matched"] for card in three) < 2 and (located := locate_card_row(frame, 3)):
        located_layout = {**layout, "cards": located}
        metrics = probe_augment_cards(rect, located_layout, frame)
        return located_layout, [{**card, "matched": True} for card in metrics]
    return layout, three


def create_engine() -> Any:
    if os.name == "nt":
        # A Conda-based Python can preload its older MSVC runtime from the
        # Python installation directory. ONNX Runtime may then fail during
        # DLL initialization, so load the Windows C++ runtime before ORT.
        system32 = Path(os.environ.get("SystemRoot", r"C:\Windows")) / "System32"
        for runtime_name in ("msvcp140.dll", "msvcp140_1.dll"):
            runtime_path = system32 / runtime_name
            if runtime_path.is_file():
                ctypes.WinDLL(str(runtime_path))

    from rapidocr import RapidOCR

    return RapidOCR(params={
        # These are narrow title strips, not full-page images. The default
        # minimum-side resize blows them up to several thousand pixels wide.
        "Det.limit_type": "max",
        "Det.limit_side_len": 640,
        # Small batches share the CPU with the game; avoid three model sessions
        # each creating a machine-sized thread pool.
        "EngineConfig.onnxruntime.intra_op_num_threads": 2,
        "EngineConfig.onnxruntime.inter_op_num_threads": 1,
    })


def recognize_cards(engine: Any, images: list[Image.Image]) -> list[tuple[str, float]]:
    from rapidocr.main import RapidOCRError

    card_regions: list[list[Any]] = []
    regions = []
    detect_started = time.perf_counter()
    for image in images:
        source = engine.load_img(image)
        prepared, operation = engine.preprocess_img(source)
        try:
            with contextlib.redirect_stdout(sys.stderr):
                crops, _ = engine.detect_and_crop(prepared, operation)
        except RapidOCRError:
            crops = []
        card_regions.append(crops)
        regions.extend(crops)
    detect_ms = (time.perf_counter() - detect_started) * 1000

    if not regions:
        return [("", 0.0) for _ in images]

    with contextlib.redirect_stdout(sys.stderr):
        classify_started = time.perf_counter()
        oriented, _ = engine.cls_and_rotate(regions)
        classify_ms = (time.perf_counter() - classify_started) * 1000
        recognize_started = time.perf_counter()
        result = engine.recognize_txt(oriented)
        recognize_ms = (time.perf_counter() - recognize_started) * 1000

    output = []
    offset = 0
    for crops in card_regions:
        count = len(crops)
        texts = [str(text).strip() for text in (result.txts or ())[offset : offset + count]]
        scores = [float(score) for score in result.scores[offset : offset + count]]
        offset += count
        texts = [text for text in texts if text]
        confidence = sum(scores) / len(scores) if scores else 0.0
        output.append(("".join(texts), max(0.0, min(confidence, 1.0))))

    print(
        f"海克斯 OCR 批处理：检测 {detect_ms:.0f} ms，方向校正 {classify_ms:.0f} ms，文字识别 {recognize_ms:.0f} ms",
        file=sys.stderr,
        flush=True,
    )
    return output


def shop_is_visible(image: Image.Image) -> bool:
    import cv2
    import numpy as np

    frame = np.asarray(image)
    height, width = frame.shape[:2]
    scale = min(1.0, 1200 / width)
    frame = cv2.resize(frame, (round(width * scale), round(height * scale)),
                       interpolation=cv2.INTER_AREA)
    height, width = frame.shape[:2]
    r, g, b = cv2.split(frame)
    gold = cv2.inRange(frame, (60, 35, 0), (225, 205, 158))
    gold[(g > r) | (b.astype(np.int16) + 8 >= g)] = 0

    def lines(horizontal: bool):
        length = max(5, round((width if horizontal else height) * .15))
        gap = (15, 1) if horizontal else (1, 15)
        shape = (length, 1) if horizontal else (1, length)
        connected = cv2.morphologyEx(gold, cv2.MORPH_CLOSE, cv2.getStructuringElement(cv2.MORPH_RECT, gap))
        filtered = cv2.morphologyEx(connected, cv2.MORPH_OPEN, cv2.getStructuringElement(cv2.MORPH_RECT, shape))
        contours, _ = cv2.findContours(filtered, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
        return [cv2.boundingRect(contour) for contour in contours]

    horizontal = lines(True)
    vertical = lines(False)
    for x, y, _, h in vertical:
        if h < height * .46:
            continue
        tolerance = max(5, round(width * .025))
        for left, edge, length, _ in horizontal:
            at_top = abs(edge - y) <= tolerance * 2
            at_bottom = abs(edge - (y + h)) <= tolerance * 2
            if length < width * .36 or not (at_top or at_bottom):
                continue
            if not 1.15 <= length / h <= 1.75:
                continue
            if abs(left + length - x) > tolerance * 2:
                continue
            opposite = y + h if at_top else y
            has_other_edge = any(
                abs(other_edge - opposite) <= tolerance
                and abs(other_left + other_length - x) <= tolerance * 2
                and other_length >= length * .7
                for other_left, other_edge, other_length, _ in horizontal
            ) or any(
                abs(other_x - left) <= tolerance
                and abs(other_y - y) <= tolerance * 2
                and other_height >= h * .8
                for other_x, other_y, _, other_height in vertical
            )
            if not has_other_edge:
                continue
            # The shop has a dark blue panel inside its top-right corner.
            sample_y = edge + 30 if at_top else edge - 100
            interior = frame[max(0, sample_y):max(0, sample_y + 70), max(0, x - 90):max(0, x - 12), :3]
            if interior.size:
                red, green, blue = np.median(interior, axis=(0, 1))
                if blue < 100 and blue >= green + 3 and green >= red + 6:
                    return True
    return False


def augment_tooltip(frame: Image.Image):
    import cv2
    import numpy as np

    top = round(frame.height * .69)
    band = np.asarray(frame.crop((0, top, frame.width, round(frame.height * .88))))
    dark = cv2.inRange(band, (0, 0, 0), (21, 23, 27))
    contours, _ = cv2.findContours(dark, cv2.RETR_EXTERNAL, cv2.CHAIN_APPROX_SIMPLE)
    for contour in sorted(contours, key=cv2.contourArea, reverse=True):
        x, y, width, height = cv2.boundingRect(contour)
        if not (.20 < x / frame.width < .65 and .22 < width / frame.width < .55
                and 1 < y < frame.height * .10 and height > frame.height * .09
                and cv2.contourArea(contour) > width * height * .9):
            continue
        y += top
        unit = frame.height / 2029
        def crop(left, upper, right, lower):
            return frame.crop(tuple(round(value) for value in (
                x + left * unit, y + upper * unit, x + right * unit, y + lower * unit,
            )))
        return crop(26, 24, 113, 113), crop(130, 8, 690, 76)
    return None


def hud_icon_score(frame: Image.Image, icon: Image.Image) -> float:
    import cv2
    import numpy as np

    hud = np.asarray(frame.crop((
        round(frame.width * .21), round(frame.height * .87),
        round(frame.width * .39), frame.height,
    )))
    template = np.asarray(icon)
    # Normalize the comparison to the captured tooltip icon size. HUD and
    # tooltip icons can have different UI scales.
    scores = []
    for scale in (.62, .70, .78, .86, .94, 1.02):
        size = max(8, round(icon.width * scale))
        if size <= min(hud.shape[:2]):
            resized = cv2.resize(template, (size, size), interpolation=cv2.INTER_AREA)
            scores.append(float(cv2.matchTemplate(hud, resized, cv2.TM_CCOEFF_NORMED).max()))
    return max(scores, default=0.0)


class HudSelection:
    def __init__(self):
        self.reset()

    def reset(self):
        self.window = None
        self.tooltip = None
        self.misses = 0
        self.hits = 0

    def observe(self, window, frame, choice, engine):
        if self.window != window:
            self.reset()
            self.window = window
        if choice:
            if self.misses >= 3:
                self.tooltip = None
            self.misses = self.hits = 0
            if tooltip := augment_tooltip(frame):
                self.tooltip = tooltip
            return None
        self.misses += 1
        if self.tooltip is None or self.misses > 3 or engine is None:
            return None
        icon, title = self.tooltip
        self.hits = self.hits + 1 if hud_icon_score(frame, icon) >= .80 else 0
        if self.misses != 3 or self.hits < 2:
            return None
        self.tooltip = None
        [(name, confidence)] = recognize_cards(engine, [title])
        return name if confidence >= .85 else None


def process_request(
    request: dict[str, Any],
    layout: dict[str, Any],
    engine_holder: list[Any],
    card_cache: dict[int, tuple[str, str, float]],
    hud_selection=None,
) -> dict[str, Any]:
    action = request.get("action")
    if action == "warmup":
        if engine_holder[0] is None:
            started = time.perf_counter()
            with contextlib.redirect_stdout(sys.stderr):
                engine = create_engine()
                # RapidOCR loads each ONNX model on first use. Load them while the
                # game is starting so the first augment choice does not pay this cost.
                engine._load_det_model()
                engine._load_cls_model()
                engine._load_rec_model()
                engine_holder[0] = engine
            elapsed = (time.perf_counter() - started) * 1000
            print(f"OCR 模型预热完成：{elapsed:.0f} ms", file=sys.stderr, flush=True)
        return {"ready": True}
    if action == "probe_augment_screen":
        try:
            started = time.perf_counter()
            rect = active_game_client_rect()
            window = rect.pop("window_handle", None)
            reference = reference_rect(rect)
            screenshot = capture_rgb(rect)
            frame = screenshot if reference == rect else crop_region(screenshot, rect, reference)
            detected_layout, cards = detect_augment_layout(reference, layout, frame)
            required_cards = max(2, len(detected_layout["cards"]) - 1)
            choice = sum(card["matched"] for card in cards) >= required_cards
            shop_visible = not choice and shop_is_visible(screenshot)
            response = {
                "game_foreground": True,
                "augment_choice": choice,
                "card_metrics": [] if shop_visible else cards,
                "shop_visible": shop_visible,
                "game_window": rect,
            }
            if hud_selection is not None:
                if shop_visible:
                    hud_selection.reset()
                else:
                    response["hud_selected_name"] = hud_selection.observe(window, frame, response["augment_choice"], engine_holder[0])
            elapsed = (time.perf_counter() - started) * 1000
            if elapsed >= 100:
                print(f"海克斯画面探测：{elapsed:.0f} ms", file=sys.stderr, flush=True)
            return response
        except RuntimeError:
            if hud_selection is not None:
                hud_selection.reset()
            return {"game_foreground": False, "augment_choice": False}

    if action not in ("recognize_current_screen", "recognize_augment_choice"):
        raise RuntimeError("不支持的 OCR 请求。")
    if request.get("language") != "ch":
        raise RuntimeError("OCR worker 只接受简体中文配置。")

    rect = active_game_client_rect()
    window_handle = rect.pop("window_handle", None)
    monitor = containing_monitor(rect)
    reference = reference_rect(rect)
    frame = capture_rgb(reference)
    layout, metrics = detect_augment_layout(reference, layout, frame)
    choice = sum(card["matched"] for card in metrics) >= max(2, len(layout["cards"]) - 1)
    if action == "recognize_augment_choice" and not choice:
        return {"augment_choice": False}
    if engine_holder[0] is None:
        with contextlib.redirect_stdout(sys.stderr):
            engine_holder[0] = create_engine()

    started = time.perf_counter()
    predictions_by_slot: dict[int, tuple[str, float]] = {}
    changed_slots = []
    changed_signatures = []
    changed_crops = []
    upscale = int(layout.get("upscale", 3))
    for card in layout["cards"]:
        slot = int(card["slot"])
        image, signature = crop_title(reference, card, layout["title_crop"], 1, frame)
        signature = f'{len(layout["cards"])}:{signature}'
        cached = card_cache.get(slot)
        if cached is not None and cached[0] == signature:
            predictions_by_slot[slot] = (cached[1], cached[2])
            continue
        if upscale > 1:
            image = image.resize(
                (image.width * upscale, image.height * upscale), Image.Resampling.LANCZOS
            )
        changed_slots.append(slot)
        changed_signatures.append(signature)
        changed_crops.append(image)

    changed_predictions = recognize_cards(engine_holder[0], changed_crops) if changed_crops else []
    for slot, signature, (raw_text, confidence) in zip(
        changed_slots, changed_signatures, changed_predictions
    ):
        predictions_by_slot[slot] = (raw_text, confidence)
        if raw_text:
            card_cache[slot] = (signature, raw_text, confidence)

    cards = []
    for card in layout["cards"]:
        raw_text, confidence = predictions_by_slot.get(int(card["slot"]), ("", 0.0))
        cards.append(
            {"slot": int(card["slot"]), "raw_text": raw_text, "confidence": confidence,
             "center_x": card["x"] + card["width"] / 2}
        )
    elapsed = (time.perf_counter() - started) * 1000
    print(f"海克斯 OCR 更新 {len(changed_crops)}/{len(cards)} 张卡片：{elapsed:.0f} ms", file=sys.stderr, flush=True)
    return {
        "augment_choice": choice,
        "screen_width": rect["width"],
        "screen_height": rect["height"],
        "monitor": monitor,
        "game_window": rect,
        "game_window_handle": window_handle,
        "cards": cards,
    }


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--layout", required=True)
    args = parser.parse_args()
    set_per_monitor_dpi_awareness()
    layout = json.loads(Path(args.layout).read_text(encoding="utf-8"))
    engine_holder: list[Any] = [None]
    card_cache: dict[int, tuple[str, str, float]] = {}
    hud_selection = HudSelection()
    for line in sys.stdin:
        try:
            request = json.loads(line)
            response = process_request(request, layout, engine_holder, card_cache, hud_selection)
        except Exception as error:  # The JSON-lines process must survive one bad capture.
            response = {"error": str(error)}
        print(json.dumps(response, ensure_ascii=True), flush=True)


if __name__ == "__main__":
    main()
