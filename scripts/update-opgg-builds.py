"""Refresh public Mayhem build references. Keeps existing rows on fetch errors."""
import concurrent.futures
import json
import os
import re
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
ROUTES = ROOT / "data/seed/build-routes.json"
BASE = "https://www.op.gg/lol/modes/aram-mayhem"

def table(html, heading):
    match = re.search(r">" + re.escape(heading) + r"</th>.*?<tbody[^>]*>(.*?)</tbody>", html, re.S)
    if not match:
        return []
    return [ids for row in re.findall(r"<tr\b.*?</tr>", match[1], re.S)
            if (ids := re.findall(r'/item/(\d+)\.png', row))]

def fetch(entry):
    hero_id, alias = entry
    slug = alias.lower()
    url = f"{BASE}/{slug}/build"
    try:
        command = ["curl.exe", "-fsSL", "--connect-timeout", "12", "--max-time", "30"]
        if proxy := os.environ.get("OPGG_PROXY"):
            command += ["--proxy", proxy]
        html = subprocess.check_output(command + [url], timeout=35).decode("utf-8")
        patch = re.search(r"Patch (\d+\.\d+)", html)
        starters, core = table(html, "Starter items"), table(html, "Core builds")
        if not patch or not starters or not core:
            raise ValueError("missing patch or build table")
        return hero_id, {"patch": patch[1], "source": url, "starters": starters, "core": core}
    except (subprocess.SubprocessError, ValueError) as error:
        print(f"{hero_id} {alias}: {error}", flush=True)
        return hero_id, None

def save(routes):
    with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=ROUTES.parent,
                                     suffix=".tmp", delete=False) as handle:
        path = Path(handle.name)
        json.dump(routes, handle, ensure_ascii=False, indent=2, sort_keys=True)
        handle.write("\n")
    for attempt in range(20):
        try:
            path.replace(ROUTES)
            return
        except PermissionError:
            if attempt == 19:
                raise
            time.sleep(0.25)

def main():
    roster = json.loads((ROOT / "data/seed/aramhex/heroes.json").read_text(encoding="utf-8"))
    source = (ROOT / "src/features/draft-advice.ts").read_text(encoding="utf-8-sig")
    entries = [(i, a) for i, a in re.findall(r"^(\d+) (\w+) [012]{5}[PMXB]$", source, re.M) if i in roster]
    if len({i for i, _ in entries}) != len(roster):
        raise ValueError("hero alias roster incomplete")
    routes = json.loads(ROUTES.read_text(encoding="utf-8"))
    pending_file = ROUTES.with_suffix(".tmp")
    if pending_file.exists():
        routes.update(json.loads(pending_file.read_text(encoding="utf-8")))
        save(routes)
        pending_file.unlink()
    if "--missing" in sys.argv:
        entries = [entry for entry in entries if entry[0] not in routes]
    refreshed = 0
    with concurrent.futures.ThreadPoolExecutor(max_workers=4) as pool:
        futures = [pool.submit(fetch, entry) for entry in entries]
        for future in concurrent.futures.as_completed(futures):
            hero_id, result = future.result()
            if result:
                routes[hero_id] = result
                refreshed += 1
                save(routes)
    pending = sorted(set(roster) - routes.keys(), key=int)
    save(routes)
    print(f"Updated {refreshed}; coverage {len(roster)-len(pending)}/{len(roster)}; missing: {pending}")

if __name__ == "__main__":
    main()
