CREATE TABLE labels (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  canonical_name TEXT NOT NULL,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  UNIQUE(workspace_id, canonical_name)
);

CREATE TABLE note_labels (
  note_id TEXT NOT NULL REFERENCES notes(id) ON DELETE CASCADE,
  label_id TEXT NOT NULL REFERENCES labels(id) ON DELETE CASCADE,
  PRIMARY KEY(note_id, label_id)
);

CREATE INDEX note_labels_label_id_idx ON note_labels(label_id);
CREATE VIRTUAL TABLE note_search USING fts5(note_id UNINDEXED, workspace_id UNINDEXED, content);

INSERT INTO note_search(note_id, workspace_id, content)
SELECT notes.id, notes.workspace_id, notes.markdown || ' ' || COALESCE(notes.annotation, '')
FROM notes;
