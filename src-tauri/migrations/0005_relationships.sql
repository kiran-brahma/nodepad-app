-- One Relationship row per unordered pair of Notes. `note_id_a` always sorts
-- before `note_id_b`, so a pair has exactly one canonical row and the unique
-- index rejects a duplicate written in either endpoint order. Nothing here
-- reads as direction: the ordering is storage, not meaning.
--
-- Both endpoints and the Workspace cascade, so a deleted Note takes its
-- Relationships with it and the Thinking Graph can never observe an endpoint
-- that no longer names a Note.
CREATE TABLE relationships (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  note_id_a TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  note_id_b TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  -- Manual is all this slice writes; AI is admitted now so a later assistance
  -- slice adds rows instead of changing what an existing row means.
  provenance TEXT NOT NULL DEFAULT 'manual' CHECK(provenance IN ('manual', 'ai')),
  created_at TEXT NOT NULL,
  CHECK(note_id_a < note_id_b),
  UNIQUE(note_id_a, note_id_b)
);

CREATE INDEX relationships_workspace_id_idx ON relationships(workspace_id);
-- `UNIQUE(note_id_a, note_id_b)` already indexes the first endpoint.
CREATE INDEX relationships_note_id_b_idx ON relationships(note_id_b);
