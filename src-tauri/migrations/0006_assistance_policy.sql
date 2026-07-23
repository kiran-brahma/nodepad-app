-- Adds the per-Workspace Assistance Policy and the locally selected model.
-- Both are non-secret preferences; they survive restart with the Workspace.
ALTER TABLE thinking_workspaces ADD COLUMN assistance_policy TEXT NOT NULL DEFAULT 'manual'
  CHECK(assistance_policy IN ('manual', 'local_ai', 'cloud_ai'));

ALTER TABLE thinking_workspaces ADD COLUMN selected_model TEXT;
