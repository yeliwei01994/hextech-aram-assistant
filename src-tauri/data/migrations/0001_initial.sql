PRAGMA foreign_keys = ON;
PRAGMA user_version = 1;

CREATE TABLE IF NOT EXISTS champions (
  champion_id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  portrait TEXT,
  patch TEXT
);

CREATE TABLE IF NOT EXISTS items (
  item_id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  tags_json TEXT NOT NULL DEFAULT '[]',
  patch TEXT
);

CREATE TABLE IF NOT EXISTS augments (
  augment_id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  icon TEXT,
  patch TEXT
);

CREATE TABLE IF NOT EXISTS champion_rankings (
  champion_id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL,
  position INTEGER,
  total INTEGER,
  badge_tier INTEGER,
  patch TEXT,
  source TEXT,
  updated_at TEXT,
  FOREIGN KEY(champion_id) REFERENCES champions(champion_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS champion_augment_grades (
  champion_id TEXT NOT NULL,
  augment_id TEXT NOT NULL,
  name TEXT NOT NULL,
  grade TEXT NOT NULL CHECK (grade IN ('S', 'A', 'B')),
  reason TEXT NOT NULL,
  patch TEXT,
  PRIMARY KEY(champion_id, augment_id),
  FOREIGN KEY(champion_id) REFERENCES champions(champion_id) ON DELETE CASCADE,
  FOREIGN KEY(augment_id) REFERENCES augments(augment_id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS recommendation_rules (
  rule_id TEXT PRIMARY KEY NOT NULL,
  trigger_tag TEXT NOT NULL,
  item_tag TEXT NOT NULL,
  reason TEXT NOT NULL,
  priority INTEGER NOT NULL DEFAULT 50
);

CREATE TABLE IF NOT EXISTS snapshot_metadata (
  key TEXT PRIMARY KEY NOT NULL,
  source TEXT NOT NULL,
  patch TEXT,
  updated_at TEXT,
  stale INTEGER NOT NULL DEFAULT 0 CHECK (stale IN (0, 1))
);

CREATE TABLE IF NOT EXISTS app_settings (
  key TEXT PRIMARY KEY NOT NULL,
  value TEXT NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_rankings_position ON champion_rankings(position);
CREATE INDEX IF NOT EXISTS idx_augment_grade ON champion_augment_grades(champion_id, grade);
