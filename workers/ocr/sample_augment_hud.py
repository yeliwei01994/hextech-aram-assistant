"""Collect local HUD crops around one confirmed augment-screen transition."""

import argparse
import json
import time
from pathlib import Path

from worker import active_game_client_rect, capture_rgb, detect_augment_layout, reference_rect


def crop_hud(frame):
    # The augment and buff row is above the central ability bar.
    return frame.crop((
        round(frame.width * 0.20), round(frame.height * 0.69),
        round(frame.width * 0.80), round(frame.height * 0.94),
    ))


def collect(layout, timeout):
    deadline = time.monotonic() + timeout
    before = None
    previous_size = None
    visible_frames = 0
    after = []
    while time.monotonic() < deadline:
        try:
            rect = active_game_client_rect()
        except RuntimeError:
            before, previous_size, visible_frames = None, None, 0
            after.clear()
            time.sleep(0.25)
            continue

        reference = reference_rect(rect)
        frame = capture_rgb(reference)
        if previous_size != frame.size:
            before, visible_frames = None, 0
            after.clear()
            previous_size = frame.size
        detected, metrics = detect_augment_layout(reference, layout, frame)
        visible = sum(card["matched"] for card in metrics) >= max(2, len(detected["cards"]) - 1)
        if visible:
            visible_frames += 1
            if visible_frames >= 2:
                before = crop_hud(frame)
            after.clear()
        else:
            visible_frames = 0
            if before is not None:
                after.append(crop_hud(frame))
                if len(after) == 5:
                    return before, after
        time.sleep(0.25)
    return None


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", required=True, type=Path)
    parser.add_argument("--timeout", type=int, default=120)
    args = parser.parse_args()
    if args.timeout < 1:
        parser.error("--timeout must be positive")

    layout = json.loads(Path(__file__).with_name("card-layout.json").read_text(encoding="utf-8"))
    from worker import set_per_monitor_dpi_awareness
    set_per_monitor_dpi_awareness()
    result = collect(layout, args.timeout)
    if result is None:
        parser.exit(1, "No confirmed select-screen closure captured.\n")
    before, after = result
    args.output.mkdir(parents=True, exist_ok=False)
    before.save(args.output / "before.png")
    for index, frame in enumerate(after, 1):
        frame.save(args.output / f"after-{index}.png")
    print(f"Saved one before and {len(after)} after HUD crops to {args.output}")


if __name__ == "__main__":
    main()
