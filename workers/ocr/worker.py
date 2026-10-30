from __future__ import annotations

import argparse
import contextlib
import ctypes
import json
import os
import sys
from ctypes import wintypes
from pathlib import Path
from typing import Any

from mss import mss
from PIL import Image


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


def crop_title(rect: dict[str, int], card: dict[str, Any], title: dict[str, float], upscale: int) -> Image.Image:
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
    if upscale > 1:
        image = image.resize(
            (image.width * upscale, image.height * upscale), Image.Resampling.LANCZOS
        )
    return image


def looks_like_augment_cards(rect: dict[str, int], layout: dict[str, Any]) -> bool:
    detected = 0
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
            image = image.resize((32, 48), Image.Resampling.BILINEAR)
            pixels = image.load()

            # The card's luminous frame sits a few pixels inside its dark outer edge.
            border_pixels = []
            border_pixels.extend(pixels[x, y] for y in range(0, 2) for x in range(7, 25))
            border_pixels.extend(pixels[x, y] for y in range(46, 48) for x in range(7, 25))
            border_pixels.extend(pixels[x, y] for x in range(0, 2) for y in range(8, 40))
            border_pixels.extend(pixels[x, y] for x in range(30, 32) for y in range(8, 40))
            warm_border_ratio = sum(
                red > 130 and green > 105 and blue > 65 and red >= green >= blue
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
            if warm_border_ratio >= 0.35 and dark_center_ratio >= 0.62:
                detected += 1
    return detected == len(layout["cards"])


def create_engine() -> Any:
    from rapidocr import RapidOCR

    return RapidOCR()


def recognize_card(engine: Any, image: Image.Image) -> tuple[str, float]:
    with contextlib.redirect_stdout(sys.stderr):
        result = engine(image)
    result_texts = result.txts if result.txts is not None else []
    result_scores = result.scores if result.scores is not None else []
    texts = [str(text).strip() for text in result_texts if str(text).strip()]
    scores = [float(score) for score in result_scores]
    confidence = sum(scores) / len(scores) if scores else 0.0
    return "".join(texts), max(0.0, min(confidence, 1.0))


def process_request(request: dict[str, Any], layout: dict[str, Any], engine_holder: list[Any]) -> dict[str, Any]:
    action = request.get("action")
    if action == "warmup":
        if engine_holder[0] is None:
            with contextlib.redirect_stdout(sys.stderr):
                engine_holder[0] = create_engine()
        return {"ready": True}
    if action == "probe_augment_screen":
        try:
            rect = active_game_client_rect()
            return {
                "game_foreground": True,
                "augment_choice": looks_like_augment_cards(reference_rect(rect), layout),
            }
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

    cards = []
    for card in layout["cards"]:
        image = crop_title(reference_rect(rect), card, layout["title_crop"], int(layout.get("upscale", 3)))
        raw_text, confidence = recognize_card(engine_holder[0], image)
        cards.append(
            {"slot": int(card["slot"]), "raw_text": raw_text, "confidence": confidence}
        )
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
    for line in sys.stdin:
        try:
            request = json.loads(line)
            response = process_request(request, layout, engine_holder)
        except Exception as error:  # The JSON-lines process must survive one bad capture.
            response = {"error": str(error)}
        print(json.dumps(response, ensure_ascii=True), flush=True)


if __name__ == "__main__":
    main()
