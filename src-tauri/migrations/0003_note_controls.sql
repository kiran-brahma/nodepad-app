-- Widens the Note Type check to the fixed set and adds Annotation plus the
-- provenance of both, so a later AI slice can never silently overwrite a value
-- the thinker set by hand. SQLite cannot relax a CHECK in place, so the table is
-- rebuilt and copied.
CREATE TABLE notes_with_annotation (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  markdown TEXT NOT NULL CHECK(length(trim(markdown)) > 0),
  note_type TEXT NOT NULL DEFAULT 'general' CHECK(note_type IN (
    'claim', 'question', 'task', 'idea', 'entity', 'quote', 'reference',
    'definition', 'opinion', 'reflection', 'narrative', 'comparison',
    'thesis', 'general'
  )),
  note_type_provenance TEXT NOT NULL DEFAULT 'default' CHECK(note_type_provenance IN ('default', 'manual')),
  annotation TEXT CHECK(annotation IS NULL OR length(trim(annotation)) > 0),
  annotation_provenance TEXT NOT NULL DEFAULT 'default' CHECK(annotation_provenance IN ('default', 'manual')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1))
);

INSERT INTO notes_with_annotation (
  id, workspace_id, markdown, note_type, note_type_provenance,
  annotation, annotation_provenance, created_at, updated_at, pinned
)
SELECT id, workspace_id, markdown, note_type, 'default', NULL, 'default', created_at, updated_at, pinned
FROM notes;

DROP TABLE notes;
ALTER TABLE notes_with_annotation RENAME TO notes;

CREATE INDEX IF NOT EXISTS notes_workspace_id_idx ON notes(workspace_id);
