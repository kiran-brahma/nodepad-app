-- Note Organization (Prompt A) bookkeeping.
--
-- A per-Note revision counter is bumped on every commit that touches the
-- Note (text, type, annotation, pin, label, relationship). The Enrichment
-- Workflow captures the revision at request time and refuses to apply a
-- result that names a different revision, so a thinker editing during
-- inference cannot have their fresh state clobbered by a stale response.
--
-- A `last_enriched_at` column records the moment the most recent successful
-- enrichment was applied, so the UI can show that a Note was organized by
-- AI and the UI's "re-enrich" affordance can render its staleness.
--
-- The provenance CHECK constraints are widened to admit the new `ai`
-- value alongside the existing `default` and `manual`. SQLite cannot
-- relax a CHECK in place, so the table is rebuilt and copied.
CREATE TABLE notes_with_enrichment (
  id TEXT PRIMARY KEY NOT NULL,
  workspace_id TEXT NOT NULL REFERENCES thinking_workspaces(id) ON DELETE CASCADE,
  markdown TEXT NOT NULL CHECK(length(trim(markdown)) > 0),
  note_type TEXT NOT NULL DEFAULT 'general' CHECK(note_type IN (
    'claim', 'question', 'task', 'idea', 'entity', 'quote', 'reference',
    'definition', 'opinion', 'reflection', 'narrative', 'comparison',
    'thesis', 'general'
  )),
  note_type_provenance TEXT NOT NULL DEFAULT 'default' CHECK(note_type_provenance IN ('default', 'manual', 'ai')),
  annotation TEXT CHECK(annotation IS NULL OR length(trim(annotation)) > 0),
  annotation_provenance TEXT NOT NULL DEFAULT 'default' CHECK(annotation_provenance IN ('default', 'manual', 'ai')),
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL,
  pinned INTEGER NOT NULL DEFAULT 0 CHECK(pinned IN (0, 1)),
  enrichment_revision INTEGER NOT NULL DEFAULT 0,
  last_enriched_at TEXT
);

INSERT INTO notes_with_enrichment (
  id, workspace_id, markdown, note_type, note_type_provenance,
  annotation, annotation_provenance, created_at, updated_at, pinned,
  enrichment_revision, last_enriched_at
)
SELECT id, workspace_id, markdown, note_type, note_type_provenance,
       annotation, annotation_provenance, created_at, updated_at, pinned,
       0, NULL
FROM notes;

DROP TABLE notes;
ALTER TABLE notes_with_enrichment RENAME TO notes;

CREATE INDEX IF NOT EXISTS notes_workspace_id_idx ON notes(workspace_id);
CREATE INDEX IF NOT EXISTS notes_enrichment_revision_idx ON notes(enrichment_revision);
