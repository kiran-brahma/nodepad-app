-- Existing Cloud AI Workspaces keep the only provider they previously had.
ALTER TABLE thinking_workspaces
  ADD COLUMN cloud_provider TEXT NOT NULL DEFAULT 'ollama';
