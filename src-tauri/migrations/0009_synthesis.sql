-- Provisional Synthesis (Prompt B) storage.
--
-- A Synthesis is provisional: it lives beside the Thinking Workspace until
-- the thinker accepts it as a thesis Note or dismisses it. Nothing here
-- mutates a source Note, and no Relationship row is ever written for a
-- pending Synthesis, so the Thinking Graph can render one distinctly
-- without a Relationship outliving a result that may still be dismissed.
--
-- `pending_synthesis_sources` keeps the Note identity *and* the enrichment
-- revision the Note carried when the Synthesis was proposed. A source that
-- was edited, deleted, or moved to another Workspace no longer matches, and
-- the snapshot marks the Synthesis stale rather than silently offering a
-- result built from material that no longer exists.
--
-- `synthesis_history` is the bounded novelty history. A row is written when
-- a Synthesis is proposed, so dismissing removes the pending content while
-- the text still keeps a later attempt from repeating it.
--
-- `synthesis_attempts` is the per-Workspace eligibility checkpoint: when the
-- last attempt ran (the cooldown clock) and how many organized Notes existed
-- then (the five-new-Notes checkpoint). A `found: false` attempt updates this
-- row exactly like a successful one, because a no-op is a successful result.

CREATE TABLE pending_syntheses (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  text TEXT NOT NULL CHECK(length(trim(text)) > 0),
  model TEXT NOT NULL,
  policy TEXT NOT NULL CHECK(policy IN ('local_ai', 'cloud_ai')),
  created_at TEXT NOT NULL
);

CREATE INDEX pending_syntheses_workspace_idx ON pending_syntheses(workspace_id);

CREATE TABLE pending_synthesis_sources (
  synthesis_id TEXT NOT NULL REFERENCES pending_syntheses(id) ON DELETE CASCADE,
  -- Deliberately not a foreign key: a deleted source Note must leave the
  -- pending Synthesis visible and stale rather than quietly shrinking it.
  note_id TEXT NOT NULL,
  note_revision INTEGER NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY (synthesis_id, note_id)
);

CREATE TABLE pending_synthesis_labels (
  synthesis_id TEXT NOT NULL REFERENCES pending_syntheses(id) ON DELETE CASCADE,
  name TEXT NOT NULL,
  position INTEGER NOT NULL,
  PRIMARY KEY (synthesis_id, position)
);

CREATE TABLE synthesis_history (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  text TEXT NOT NULL,
  created_at TEXT NOT NULL
);

CREATE INDEX synthesis_history_workspace_idx ON synthesis_history(workspace_id, created_at);

CREATE TABLE synthesis_attempts (
  workspace_id TEXT PRIMARY KEY NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  last_attempt_at TEXT NOT NULL,
  organized_note_checkpoint INTEGER NOT NULL
);
