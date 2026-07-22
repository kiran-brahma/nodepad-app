CREATE TABLE IF NOT EXISTS thinking_workspaces (
  id TEXT PRIMARY KEY NOT NULL,
  name TEXT NOT NULL CHECK(length(trim(name)) > 0),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS notes (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  markdown TEXT NOT NULL CHECK(length(trim(markdown)) > 0),
  note_type TEXT NOT NULL DEFAULT 'general' CHECK(note_type = 'general'),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1))
);

CREATE INDEX IF NOT EXISTS notes_workspace_id_idx ON notes(workspace_id);
