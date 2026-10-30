# ARAM Game Integration and OCR Overlay Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional automatic game-state detection, hotkeys, local augment OCR, and a non-interactive recommendation overlay while retaining manual fallbacks.

**Architecture:** Rust exposes one normalized `GameState` regardless of whether it came from LCU, Live Client Data, or manual entry. A separately packaged RapidOCR worker handles the fixed augment-card region on demand; the Tauri app renders suggestions and never clicks choices.

**Tech Stack:** Tauri 2/Rust, React/TypeScript, `tauri-plugin-global-shortcut`, RapidOCR 3.10+, PP-OCRv6 small, ONNX Runtime CPU, Python sidecar packaging.

**Spec:** `../../../outputs/aram-helper-design.md`

## Global Constraints

- Client integrations are optional; manual entry remains available in every phase.
- Do not store or display win-rate percentages.
- OCR is local, uses Simplified Chinese (`ch`), and starts only on demand.
- Never automate selection or send game input back to League.
- Register game hotkeys as `Ctrl+Shift+F6/F7/F8` chords so ordinary function keys remain available to the game.
- The provided 3840×2160 augment-pick sample calibrates three normalized card regions; capture only each title crop in memory.
- Calibrated card boxes: `x=[0.224, 0.417, 0.609]`, `y=0.177`, `width=0.168`, `height=0.491`; title crop within each card: `x=0.10`, `y=0.44`, `width=0.80`, `height=0.10`.
- Position the overlay relative to the active game's client window, then fall back to its monitor when no capture bounds exist.
- External LCU behavior may change; parsing errors must not terminate the app.

## Review Focus

- League client is closed or login state is missing: show disconnected status and keep manual workflow available.
- LCU response is malformed or changes shape: retain last known phase only as stale and do not guess current champions.
- Enemy roster is unavailable before the match begins: ask for manual entry rather than implying an opponent lineup.
- OCR returns a name with low confidence or a newly added augment: require confirmation and preserve the raw text.
- Screen resolution or UI scaling changes: capture relative card regions and allow manual correction instead of returning random augments.

---

## File Structure

- `src-tauri/src/game/{mod,state,lcu,live_client,watcher}.rs`: game phase and data-source adapters.
- `workers/ocr/capture.py`: screen capture and normalized augment-card crop using `mss`.
- `src-tauri/src/{overlay,hotkeys}.rs`: transparent topmost overlay and global shortcuts.
- `src-tauri/src/ocr/{mod,protocol,normalize}.rs`: Rust OCR worker client and augment-name normalization.
- `workers/ocr/`: Python RapidOCR worker and build configuration.
- `src/features/game/`: connection status and manually editable roster.
- `src/features/augment-picker/`: recognized cards and confirmation flow.
- `src-tauri/capabilities/default.json`: minimal permissions for overlay and sidecar execution.

## Task 1: Define the game-state boundary and manual source

**Files:**
- Create: `src-tauri/src/game/{mod,state,watcher}.rs`
- Modify: `src-tauri/src/domain/team.rs`, `src-tauri/src/commands/game.rs`, `src-tauri/src/lib.rs`
- Create: `src/features/game/GameStatus.tsx`, `src/features/game/ManualRosterEditor.tsx`

**Interfaces:**
- Consumes the shared `GamePhase` enum from `src-tauri/src/domain/game.rs` in the offline-core plan.
- `GameState { phase, player_champion_id: Option<String>, allies: Vec<String>, enemies: Vec<String>, source: "manual" | "lcu" | "live_client", allies_complete: bool, enemies_complete: bool, observed_at: String }`
- `GameStateWatcher::current() -> GameState`
- `set_manual_game_state(input: TeamBuildInput) -> GameState`

- [ ] **Step 1: Add normalized game-state types**

Keep client-specific fields out of UI DTOs. Every state includes source and observation time so stale data can be shown honestly.

- [ ] **Step 2: Implement the manual watcher**

Return `Disconnected` until a client adapter is active; manual updates replace roster fields and set `source = manual`.

- [ ] **Step 3: Connect the game status and manual roster UI**

Provide a visible connection state and preserve the hand-edited roster when auto-detection is unavailable.

**Deliverable:** All later adapters can be added without changing screens or the recommendation engine.

## Task 2: Add League client readers

**Files:**
- Create: `src-tauri/src/game/lcu.rs`, `src-tauri/src/game/live_client.rs`
- Modify: `src-tauri/src/game/watcher.rs`, `src-tauri/src/commands/game.rs`

**Interfaces:**
- `LcuReader::read_phase_and_allies() -> Result<ClientSnapshot>`
- `LiveClientReader::read_players() -> Result<ClientSnapshot>`
- `ClientSnapshot { phase, player_champion_id, allies, enemies, allies_complete, enemies_complete, observed_at }`

- [ ] **Step 1: Implement LCU connection discovery**

Read the local League client connection metadata without persisting credentials. Apply a short request timeout and convert all transport/parse failures into a disconnected status.

- [ ] **Step 2: Implement the LCU champ-select adapter**

Read only data available to the player at champion select. Do not infer hidden enemy picks.

- [ ] **Step 3: Implement the in-game Live Client Data reader**

During an active game, read the local game endpoint and map player champion IDs into allies/enemies for the existing `GameState` DTO.

- [ ] **Step 4: Add bounded polling and source switching**

Poll at a modest interval while the client is active, stop polling when disconnected, and prefer manual input whenever the reader reports an incomplete roster.

**Deliverable:** Automatic champion and in-game roster detection with manual fallback. LCU usage remains isolated because Riot does not guarantee support for the League Client API.

## Task 3: Add local augment OCR

**Files:**
- Create: `workers/ocr/{worker,capture}.py`, `workers/ocr/requirements.txt`, `workers/ocr/build.ps1`
- Create: `src-tauri/src/ocr/{mod,protocol,normalize}.rs`
- Modify: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/capabilities/default.json`

**Interfaces:**
- Worker request: `{ "action": "recognize_current_screen", "language": "ch" }`
- Worker response: `{ "screen_width": number, "screen_height": number, "cards": [{ "slot": number, "raw_text": string, "confidence": number }] }`
- `OcrClient::recognize_current_screen() -> Result<OcrScreenResult>`
- `OcrScreenResult { screen_width: u32, screen_height: u32, cards: Vec<RecognizedCard> }`
- `RecognizedCard { slot: u8, raw_text: String, confidence: f32 }`
- `match_augment_name(raw_text: &str, known_names: &[String]) -> Option<NameMatch>` runs in Rust against the local catalog; show a grade only when OCR confidence is at least `0.80` and normalized title similarity is at least `0.82`.
- `OCR_CONFIRMATION_THRESHOLD: f32 = 0.80` is the initial cutoff, stored in settings and calibrated with the supplied game screenshot.

- [ ] **Step 1: Create the OCR worker contract**

Use JSON Lines over stdin/stdout and keep logs on stderr so OCR results remain machine-readable. The worker manager starts the sidecar on the first F6 call, keeps it alive while a game is active, and stops it after the game ends or the app exits.

- [ ] **Step 2: Package RapidOCR for Simplified Chinese**

Use `rapidocr>=3.10.0`, `onnxruntime`, and `mss`; configure PP-OCRv6 `small` with language `ch`. Keep the model lazy-loaded in the worker.

- [ ] **Step 3: Capture and crop only the augment cards**

Capture to memory with `mss`, scale normalized card boxes to the current client resolution, and enlarge only the title crops. Calibrate normalized crop boxes against the user's real in-game sample screenshot before locking them in; do not persist screenshots.

- [ ] **Step 4: Normalize against the local augment catalog**

Normalize full-width spaces/punctuation, compare OCR text to known current-patch augment names, and preserve raw text when no safe match exists.

- [ ] **Step 5: Add confidence-based confirmation**

Show recognized names when confidence is sufficient; otherwise present candidate names and a manual picker. Do not silently select an augment.

**Deliverable:** On-demand local recognition that returns augment candidates with confidence and a correction path.

## Task 4: Add hotkeys and recommendation overlay

**Files:**
- Create: `src-tauri/src/{hotkeys,overlay}.rs`
- Create: `src/features/augment-picker/{AugmentOverlay,RecognizedCard}.tsx`
- Modify: `src/App.tsx`, `src-tauri/src/lib.rs`, `src-tauri/capabilities/default.json`

**Interfaces:**
- Ctrl+Shift+F6 calls `OcrClient::recognize_current_screen()`, then matches the raw title against the current hero's local grade rows.
- Ctrl+Shift+F7 refreshes the visible game-state snapshot; Ctrl+Shift+F8 hides the overlay.
- `AugmentAdvice { augment_id: String, name: String, grade: "S" | "A" | "B", reason: String, confidence: f32 }` is the overlay DTO.
- `show_overlay(cards: Vec<AugmentAdvice>) -> Result<()>` displays a non-interactive, topmost result window.

- [ ] **Step 1: Register and clean up global shortcuts**

Register F6/F7 at startup, handle duplicate registration safely, and unregister them on shutdown.

- [ ] **Step 2: Wire F6 recognition to local augment advice**

Resolve the current champion from game state; if missing, prompt the user to set it manually before looking up grades.

- [ ] **Step 3: Render the transparent topmost overlay**

Display augment name, S/A/B badge, and short reason. Keep the surface mouse-transparent and provide the F8 chord to hide it; do not click game controls.

- [ ] **Step 4: Surface graceful failure states**

Show a compact message when the client is closed, OCR worker fails, a card is unknown, or no local grade is available.

**Deliverable:** Original-style hotkey flow rebuilt on top of the new UI and local domain.

