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
    return {"left": origin.x, "top": origin.y, "width": width, "height": height}


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


def crop_title(
    rect: dict[str, int],
    card: dict[str, Any],
    title: dict[str, float],
    upscale: int,
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
    with mss() as capture:
        screenshot = capture.grab(crop)
    image = Image.frombytes("RGB", screenshot.size, screenshot.bgra, "raw", "BGRX")
    signature = title_signature(image)
    if upscale > 1:
        image = image.resize(
            (image.width * upscale, image.height * upscale), Image.Resampling.LANCZOS
        )
    return image, signature


def title_signature(image: Image.Image) -> str:
    # Titles are static while the card art and border animate. A small binary
    # mask keeps the signature focused on the text and cheap to compare.
    sample = image.convert("L").resize((32, 8), Image.Resampling.BILINEAR)
    mask = bytes(1 if pixel >= 145 else 0 for pixel in sample.getdata())
    return hashlib.blake2s(mask, digest_size=8).hexdigest()


def probe_augment_cards(rect: dict[str, int], layout: dict[str, Any]) -> list[dict[str, Any]]:
    results = []
    with mss() as capture:
        for card in layout["cards"]:
            region = {
                "left": rect["left"] + round(rect["width"] * float(card["x"])),
                "top": rect["top"] + round(rect["height"] * float(card["y"])),
                "width": round(rect["width"] * float(card["width"])),
                "height": round(rect["height"] * float(card["height"])),
            }
            frame = capture.grab(region)
            image = Image.frombytes("RGB", frame.size, frame.bgra, "raw", "BGRX")
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

    return RapidOCR()


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


def process_request(
    request: dict[str, Any],
    layout: dict[str, Any],
    engine_holder: list[Any],
    card_cache: dict[int, tuple[str, str, float]],
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
            cards = probe_augment_cards(reference_rect(rect), layout)
            required_cards = max(2, len(layout["cards"]) - 1)
            response = {
                "game_foreground": True,
                "augment_choice": sum(card["matched"] for card in cards) >= required_cards,
                "card_metrics": cards,
                "game_window": rect,
            }
            elapsed = (time.perf_counter() - started) * 1000
            if elapsed >= 100:
                print(f"海克斯画面探测：{elapsed:.0f} ms", file=sys.stderr, flush=True)
            return response
        except RuntimeError:
            return {"game_foreground": False, "augment_choice": False}

    if action != "recognize_current_screen":
        raise RuntimeError("不支持的 OCR 请求。")
    if request.get("language") != "ch":
        raise RuntimeError("OCR worker 只接受简体中文配置。")

    rect = active_game_client_rect()
    monitor = containing_monitor(rect)
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
        image, signature = crop_title(reference_rect(rect), card, layout["title_crop"], 1)
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
            {"slot": int(card["slot"]), "raw_text": raw_text, "confidence": confidence}
        )
    elapsed = (time.perf_counter() - started) * 1000
    print(f"海克斯 OCR 更新 {len(changed_crops)}/3 张卡片：{elapsed:.0f} ms", file=sys.stderr, flush=True)
    return {
        "screen_width": rect["width"],
        "screen_height": rect["height"],
        "monitor": monitor,
        "game_window": rect,
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
    for line in sys.stdin:
        try:
            request = json.loads(line)
            response = process_request(request, layout, engine_holder, card_cache)
        except Exception as error:  # The JSON-lines process must survive one bad capture.
            response = {"error": str(error)}
        print(json.dumps(response, ensure_ascii=True), flush=True)


if __name__ == "__main__":
    main()
