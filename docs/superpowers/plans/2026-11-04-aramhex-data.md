# AramHex data integration

**Goal:** Replace OP.GG automatic sync with AramHex public all-player statistics; retain S/A/B, display raw win rate, games, and statistical advice.

**Architecture:** Dedicated provider validates the public manifest and file SHA-256 values, then imports a whole dataset in one SQLite transaction. Add separate statistics tables so legacy imports remain readable. Startup and six-hour background checks use a single sync task; failed updates preserve the last complete snapshot. Keep existing OCR and detection unchanged.

**Decisions:** All-player normal mode only. Rating uses `(wins + hero_baseline * 300) / (games + 300) - hero_baseline`: S >= 2 percentage points, A >= 0, otherwise B. These are app thresholds, not upstream official tiers. Honor upstream low_sample and excluded_augments; no games means no grade. New dataset identity includes seq, patch, mode, path and file digests. Do not downgrade seq. Display source timestamp, attribution and CC BY-NC 4.0. A sync network failure marks cached data as stale.

## Tasks

- [x] Write failing Rust tests for smoothing, sample warnings, invalid data, missing/excluded statistics, atomic imports and unchanged versions.
- [x] Add provider, statistics tables, bounded downloads, pinned-revision fetches, SHA-256 verification, and auto-sync loop. Fetch and validate current public data for an offline bundled snapshot.
- [x] Route augment queries and OCR advice through statistics, add raw win rate and games, stop per-hero OP.GG downloads.
- [x] Display advice below each card without covering the title or changing detection. Update settings source, cadence and attribution.
- [x] Run Rust tests, frontend build, live dataset validation and inspect resulting UI. Review diff for fallback, mapping, migration and stale-data failures.

**Validation focus:** Existing databases; mismatched English/numeric hero IDs; corrupt/incomplete upstream updates; low-sample and excluded augments; lost network connectivity. No OCR changes, commits or external writes.

## Verification

- Public 16.21 seq 12 snapshot fetched from pinned data-v12; four files match manifest lengths and SHA-256.
- Rust tests: 19 passed, 2 existing network tests ignored. Frontend production build passed.
- Actual overlay component rendered with S/A/missing-data fixture at 1280×720 and visually inspected. Statistics remain above the card capture region. Real game validation remains user-side after restart.
- Independent review findings (future hero identity and impossible exposure counts) fixed and regression tests added; follow-up review found no blockers.

## Network follow-up

- Reproduced user startup sync failure with the actual Rust client: connection timeout. Added bounded GitHub/Fastly/Gcore fallback, candidate-level manifest/hash validation, request-local direct fallback and detailed errors; Windows proxy configuration is unchanged.
- Offline Rust suite: 20 passed, 3 ignored. Explicit live_manifest_connection test was run separately and failed: all six system/direct routes hit connection timeouts. Automatic online sync is not verified restored; the bundled/local verified snapshot remains usable.
- Independent review of download changes found no additional correctness/security blockers. Mirror caching can delay update discovery.

## Official mirror recovery

- Located original project `MIRROR = WEBSITE + "/u"` and its `/data` transport. Verified official `https://aramhex.com/u/data` works directly while GitHub/CDN routes time out.
- Prioritize direct official mirror, retain system-proxy official mirror and GitHub/CDN fallbacks. No Windows network settings changed. All data files remain size/SHA-256 checked against one manifest, even on mutable official URLs.
- Explicit Rust live_manifest_connection test passed: downloaded all four files, parsed and verified seq 12 / patch 16.21, imported 173 heroes into an in-memory database, 4.42 seconds. Earlier online failure is superseded by this successful end-to-end test.
- Follow-up independent review found no blockers.

## Authorized v0.1.1 release

User explicitly requested GitHub push, itemized updates and a portable ZIP, superseding the initial no-publication scope. Prepared v0.1.1 CHANGELOG and release notes, bundled data license/attribution, rebuilt OCR with current configuration and native optimized app.

Validation: 23 Rust tests passed, 3 optional network tests ignored; frontend and optimized native build passed. ZIP has 708 entries, 121068019 bytes; integrity checked, extracted in a separate directory, main executable is x64/version 0.1.1, extracted OCR warmed up with developer Python/Conda variables removed and Windows-only PATH. SHA-256: b12f7810ede900783d81b0a1b902135950b82d046d95b537719483794ddadbc8. Independent release review found no blockers.

Pre-existing untracked 2026-11-02 design/plan files are outside this release and remain untouched.
