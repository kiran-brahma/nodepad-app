-- Records when a Thinking Workspace first granted affirmative consent to send
-- Note content to Ollama Cloud for inference. The bearer key itself never
-- lives in the database: it is read from the macOS keychain on demand.
ALTER TABLE thinking_workspaces ADD COLUMN cloud_consent_at TEXT;
