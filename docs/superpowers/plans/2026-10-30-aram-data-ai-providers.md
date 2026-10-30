# ARAM Ranking Data and Optional AI Providers Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add optional ranking snapshot refresh/import and optional AI matchup analysis without making either dependency necessary for the offline app.

**Architecture:** Source adapters convert external data into validated local snapshots; SQLite remains the source of truth during play. The optional AI adapter receives only enemy champion names and user-selected enemy tags, then may return a fixed set of enemy threat tags. The local rule engine maps those tags to equipment directions; the model cannot return free text, item IDs or names.

**Tech Stack:** Rust async HTTP client, serde JSON, SQLite transaction replacement, Windows Credential Manager, optional OpenAI-compatible API adapter.

**Spec:** `../../../outputs/aram-helper-design.md`

## Global Constraints

- Keep all online functionality opt-in and initiated by the user.
- Never display champion or augment win-rate percentages.
- Do not bundle provider API keys; store user-supplied credentials outside the frontend.
- Validate every downloaded record against the current local champion, augment, and item catalogs.
- Preserve the last valid snapshot whenever refresh/import fails.

## Review Focus

- Provider timeout or rate-limit response: keep the cached snapshot and show its age.
- Provider schema changes or unknown fields: reject the incoming snapshot as a whole and preserve the last valid one.
- Snapshot patch mismatch: display the mismatch and do not overwrite the active snapshot silently.
- AI returns an unknown threat tag or malformed output: discard the AI result and retain local rule suggestions.
- AI is disabled or credentials are missing: make no network request and leave local recommendations unchanged.

---

## File Structure

- `src-tauri/src/providers/rankings/{mod,snapshot,json_import,opgg}.rs`: ranking source adapters and validation.
- `src-tauri/src/providers/ai/{mod,openai_compatible,credential_store}.rs`: optional analysis provider and secret storage.
- `src-tauri/src/commands/providers.rs`: user-triggered refresh/import/settings commands.
- `src/features/settings/{DataSourceSettings,AiSettings}.tsx`: source and AI controls.
- `src-tauri/src/db/snapshot.rs`: atomic promotion of validated snapshots.
- `docs/data-sources.md`: attribution, patch semantics and provider caveats.

## Task 1: Define the provider-neutral rank snapshot format

**Files:**
- Create: `src-tauri/src/providers/rankings/{mod,snapshot,json_import}.rs`
- Modify: `src-tauri/src/db/snapshot.rs`, `src-tauri/src/commands/providers.rs`
- Create: `docs/data-sources.md`, `data/schemas/rank-snapshot.schema.json`

**Interfaces:**
- `RankSnapshot { schema_version: u16, patch: String, source: String, updated_at: String, champions: Vec<ChampionRankRow> }`
- `RankProvider::fetch_snapshot(region: Option<&str>) -> Result<RankSnapshot>`
- `SnapshotRepository::promote(snapshot: RankSnapshot) -> Result<DataStatus>`
- `SnapshotRepository::import_json(contents: &str) -> Result<DataStatus>`

- [ ] **Step 1: Write the versioned snapshot schema**

Define required schema version, patch, source, timestamp, champion ranks, and badge tiers. The ranking snapshot contains no win-rate or augment-grade fields; champion-specific augment grades remain a separate local table.

- [ ] **Step 2: Implement JSON import validation**

Validate schema version, patch consistency, champion IDs, rank bounds, and badge values before opening a DB transaction.

- [ ] **Step 3: Promote snapshots atomically**

Insert the complete validated snapshot under one transaction, switch the active snapshot only after success, and retain the previous active snapshot on failure.

- [ ] **Step 4: Add source and attribution metadata**

Record provider name, patch, update time, and required attribution text; expose these fields to the settings/status UI.

**Deliverable:** A source-neutral way to keep offline rankings and augment grades current without inventing data.

## Task 2: Add a ranking provider only after source validation

**Files:**
- Create: `src-tauri/src/providers/rankings/opgg.rs`
- Modify: `src-tauri/src/providers/rankings/mod.rs`, `src-tauri/src/commands/providers.rs`, `src/features/settings/DataSourceSettings.tsx`

**Interfaces:**
- `OpggProvider::fetch_snapshot(region: Option<&str>) -> Result<RankSnapshot>`
- `refresh_rankings() -> Result<DataStatus>`

- [ ] **Step 1: Confirm an allowed, stable data endpoint**

Record the provider URL, access method, patch/update semantics, attribution requirements, and usage limits. Implement the adapter only for a permitted stable endpoint; if none is available, leave OP.GG refresh out and use the JSON import path. Keep provider code isolated from game and UI code.

- [ ] **Step 2: Map source records to local ranking rows**

Normalize champion IDs, rank position/total, badge tier, patch and source timestamp. Discard win-rate fields at ingestion rather than carrying them into storage.

- [ ] **Step 3: Add user-triggered refresh**

Refresh only when the user selects `更新数据`. Show progress, updated patch/date, and keep the previous snapshot on all fetch or validation failures.

- [ ] **Step 4: Document offline fallback**

Document that the app uses its last valid snapshot or JSON import whenever the provider is unavailable.

**Deliverable:** Ranking updates are optional and fail without breaking local ranking display or recommendations.

## Task 3: Add optional AI matchup analysis

**Files:**
- Create: `src-tauri/src/providers/ai/{mod,openai_compatible,credential_store}.rs`
- Create: `src-tauri/src/commands/ai.rs`
- Create: `src/features/settings/AiSettings.tsx`
- Modify: `src-tauri/src/recommendation/engine.rs`, `src-tauri/src/commands/builds.rs`

**Interfaces:**
- `AiMatchupRequest { player_champion_id, allies, enemies, allies_complete: bool, enemies_complete: bool, local_suggestions: Vec<BuildSuggestion>, patch }`
- `AiExplanation { inferred_enemy_tags: Vec<ThreatTag> }`
- AI response is restricted to the fixed `ThreatTag` enum and has no free-text fields.

- [ ] **Step 1: Add opt-in AI settings**

Let the user configure a compatible endpoint, model name and key. Store the key in Windows Credential Manager; keep the frontend limited to enabled/model/status fields.

- [ ] **Step 2: Define bounded prompt input and output**

Send only enemy champion names and manually selected enemy tags. Require structured JSON containing only allow-listed threat tags; do not include allies, screenshots, credentials, item names or free text.

- [ ] **Step 3: Validate and merge the response**

Reject malformed or unknown-tag outputs. Merge inferred tags into the local rule engine; AI may not create equipment names or initiate any game action.

- [ ] **Step 4: Add offline and failure behavior**

When disabled, unconfigured, timed out, or rate-limited, render the local rule recommendations unchanged and show a non-blocking status.

**Deliverable:** AI can infer enemy matchup traits and explain the resulting local equipment directions when explicitly requested. Without AI, manually selected traits still drive the offline rules.

