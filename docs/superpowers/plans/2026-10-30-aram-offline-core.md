# ARAM Offline Core and UI Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Build the local-first Windows app shell, searchable champion ranking UI, champion detail with S/A/B augment grades, and offline team-build recommendations.

**Architecture:** Tauri 2 hosts a React/TypeScript interface and a Rust core. The Rust core owns SQLite, domain types, and deterministic recommendation rules. The initial experience supports manual champion and team inputs so the app remains useful before game-client integration is added.

**Tech Stack:** Tauri 2, React, TypeScript, Vite, Rust stable, SQLite via `rusqlite`, serde JSON, CSS design tokens.

**Spec:** `../../../outputs/aram-helper-design.md`

## Global Constraints

- Target Windows 10/11 x64 and Simplified Chinese UI.
- Never display champion or augment win-rate percentages.
- Do not require a network connection, Riot account login, or cloud service to start the app.
- Keep the ranking source behind an adapter; never invent production ranking values.
- Keep manual champion and enemy-team entry available.

## Review Focus

- Empty rank snapshot: render an explicit “暂无榜单数据” state; do not show fabricated rank labels.
- Stale patch snapshot: show its patch and update date next to the ranking.
- Unknown champion or augment IDs: preserve the record and show a recoverable unknown-data state.
- Partial enemy roster: produce suggestions only from known champions and state that the roster is incomplete.
- Missing or corrupt local database: rebuild from the last valid bundled snapshot without silently replacing it with empty data.

---

## File Structure

- `package.json`, `vite.config.ts`, `index.html`: Tauri-compatible frontend development setup and scripts.
- `src/main.tsx`, `src/App.tsx`, `src/styles/tokens.css`: React entry, app shell, theme tokens.
- `src/features/rankings/`: ranking list, search, empty/stale states.
- `src/features/champions/`: champion detail and augment grade list.
- `src/features/builds/`: manual team form and recommendation cards.
- `src/lib/tauri.ts`, `src/types/domain.ts`: command bridge and frontend DTOs.
- `src-tauri/Cargo.toml`, `src-tauri/src/lib.rs`: Rust app setup and command registration.
- `src-tauri/src/domain/`: serialized domain records, including shared status and game-phase types.
- `src-tauri/src/db/`: SQLite schema, migrations, snapshot loader and repositories.
- `src-tauri/src/recommendation/`: deterministic build rules and explanations.
- `src-tauri/src/commands/`: typed UI commands.
- `data/seed/`: versioned local seed snapshots; development mock data stays outside release builds.

## Task 1: Create the Tauri application shell

**Files:**
- Create: `package.json`, `vite.config.ts`, `index.html`
- Create: `src/main.tsx`, `src/App.tsx`, `src/styles/tokens.css`
- Create: `src-tauri/Cargo.toml`, `src-tauri/tauri.conf.json`, `src-tauri/src/lib.rs`
- Create: `src-tauri/src/domain/{status,game}.rs`, `src-tauri/src/commands/status.rs`

**Interfaces:**
- Produces a single main window with a React root and a Rust command registry.
- Exposes `get_app_status() -> AppStatus` with app version, local data status, and shared game phase (`Disconnected` initially).
- Defines `DataStatus { ready: bool, source: Option<String>, patch: Option<String>, updated_at: Option<String>, stale: bool }` in `src-tauri/src/domain/status.rs`.
- Defines `GamePhase = Disconnected | ChampSelect | Loading | InGame | AugmentChoice` in `src-tauri/src/domain/game.rs`, serialized as lowercase snake case.

- [ ] **Step 1: Scaffold Tauri 2 with the React + TypeScript template**

Run `pnpm create tauri-app@latest` in the workspace root and select the Tauri 2, React, TypeScript, Vite template. Do not add an HTTP backend or bundle a Node runtime in the packaged app.

- [ ] **Step 2: Add the app status DTO and command**

Create `src-tauri/src/domain/status.rs` and `src-tauri/src/commands/status.rs`. Serialize `AppStatus { version: String, data_status: DataStatus, game_phase: GamePhase }` through Tauri. Define `GamePhase = Disconnected | ChampSelect | Loading | InGame | AugmentChoice` in `src-tauri/src/domain/game.rs`.

- [ ] **Step 3: Build the navigation shell and design tokens**

Add navigation for `英雄榜`, `英雄详情`, `阵容出装`, and `设置`. Use Simplified Chinese copy, dark neutral surfaces, restrained blue/teal accents, consistent spacing, and visible keyboard focus.

- [ ] **Step 4: Connect status to the UI**

The shell reads `get_app_status()` on startup and shows local-data and game-connection states without blocking the main screen.

**Deliverable:** A launchable desktop shell with a clear offline/connection status and no required account or network.

## Task 2: Define the local domain and SQLite snapshot

**Files:**
- Create: `src-tauri/src/domain/{champion,augment,item,team,ranking}.rs`
- Create: `src-tauri/src/db/{mod,migrations,repository,snapshot}.rs`
- Create: `src-tauri/data/migrations/0001_initial.sql`
- Create: `data/seed/schema.json`, `data/seed/augment_grades.json`
- Modify: `src-tauri/src/lib.rs`

**Interfaces:**
- `ChampionRankRow { champion_id, name, position: Option<u16>, total: Option<u16>, badge_tier: Option<u8>, patch: Option<String>, source: Option<String>, updated_at: Option<String> }`
- `ChampionDetail { champion_id, name, portrait: Option<String>, ranking: Option<ChampionRankRow>, patch: Option<String> }`
- `AugmentGradeRow { champion_id, augment_id, name, grade: "S" | "A" | "B", reason, patch }`
- `TeamBuildInput { player_champion_id, allies: Vec<String>, enemies: Vec<String>, allies_complete: bool, enemies_complete: bool }`
- `BuildSuggestion { item_id, item_name, priority: u8, reasons: Vec<String>, source: "local_rule" | "ai" }`
- `BuildAdvice { allies_complete: bool, enemies_complete: bool, suggestions: Vec<BuildSuggestion> }`
- `RankingRepository::list(query: Option<String>) -> Result<Vec<ChampionRankRow>>`
- `RankingRepository::detail(champion_id: &str) -> Result<Option<ChampionDetail>>`

- [ ] **Step 1: Create SQLite tables and migrations**

Create tables for champions, items, augments, champion rankings, champion-augment grades, patch metadata, and recommendation rules. Store source and patch metadata with each imported snapshot.

- [ ] **Step 2: Implement snapshot validation and loading**

Load the local champion/item/augment catalogs and the locally maintained `augment_grades.json` table into the user-local database in a transaction. Reject invalid IDs, duplicate keys, and unknown grades; preserve the last valid database on failure. Champion ranking values are imported separately by the provider plan.

- [ ] **Step 3: Add repository commands**

Expose `list_hero_rankings(query)`, `get_hero_detail(champion_id)`, and `get_augment_grades(champion_id)` as typed Tauri commands.

- [ ] **Step 4: Define production empty-data behavior**

When no valid ranking or grade snapshot is present, return an empty result with `DataStatus` explaining the missing source. Return unknown champion/augment records as recoverable unknown-data states rather than dropping raw IDs. Use sample values only in a development fixture excluded from release builds.

**Deliverable:** A local versioned data store with clear behavior when the rank provider has not been configured.

## Task 3: Implement offline team-build rules

**Files:**
- Create: `src-tauri/src/recommendation/{mod,features,rules,engine}.rs`
- Create: `data/seed/items.json`
- Create: `data/seed/recommendation_rules.json`
- Modify: `src-tauri/src/commands/builds.rs`, `src-tauri/src/lib.rs`

**Interfaces:**
- `recommend_build(input: TeamBuildInput) -> Result<BuildAdvice>`
- `TeamProfile::from_champions(known_champion_ids: &[String]) -> TeamProfile`
- Every `BuildSuggestion` contains an item ID resolved against the local catalog and at least one reason; `BuildAdvice` carries roster completeness to the UI.

- [ ] **Step 1: Add item tags and team-profile fields**

Represent physical/magic damage, healing, shielding, durable frontline, and range/engage traits as local tags. Unknown champions contribute no unsupported tags.

- [ ] **Step 2: Add deterministic situational rules**

Create rules that map team traits to item tags such as anti-heal, magic resistance, armor, penetration, and anti-shield. Rank suggestions by priority and retain the matched rule IDs as reasons.

- [ ] **Step 3: Expose the recommendation command**

Expose `recommend_build(input) -> BuildAdvice` and return local-rule suggestions even when no AI provider or network is configured.

- [ ] **Step 4: Implement partial-roster messaging**

When the input marks allies or enemies incomplete, include that status in `BuildAdvice` and do not present the result as a full-composition analysis.

**Deliverable:** Useful, explainable offline equipment suggestions from manual roster input.

## Task 4: Build the ranking, champion, augment and build screens

**Files:**
- Create: `src/features/rankings/{RankingPage,RankingRow}.tsx`
- Create: `src/features/champions/{ChampionPage,AugmentGradeList}.tsx`
- Create: `src/features/builds/{BuildPage,TeamEditor,BuildAdviceList}.tsx`
- Create: `src/features/settings/SettingsPage.tsx`
- Create: `src/components/{Badge,ChampionPortrait,EmptyState,StatusPill}.tsx`
- Modify: `src/App.tsx`, `src/types/domain.ts`, `src/lib/tauri.ts`

**Interfaces:**
- `RankingPage` consumes `list_hero_rankings(query)` and renders position/total and badge tier without rate fields.
- `ChampionPage` consumes `get_hero_detail(id)` and `get_augment_grades(id)`; grades render only as S/A/B with reasons.
- `BuildPage` edits `TeamBuildInput` and consumes `recommend_build(input) -> BuildAdvice`.

- [ ] **Step 1: Add the searchable ranking list**

Display `position/total`, badge tier, champion portrait and name. Make empty, stale, and loading states visible.

- [ ] **Step 2: Add the champion detail and augment list**

Group augments by S, A, and B. Show the short fit reason on each row and never render win-rate percentages.

- [ ] **Step 3: Add manual team composition input**

Allow the player champion, allies, and up to five enemies to be searched and edited. Show local recommendation reasons and incomplete-roster status.

- [ ] **Step 4: Add settings and source status**

Display active patch, source, last update, client connection state, and whether optional AI is enabled.

**Deliverable:** An offline-first UI that remains usable with manual inputs and real locally imported data.

