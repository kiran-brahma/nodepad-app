-- Spatial placement belongs to a Note but remains optional so every existing
-- Thinking Workspace opens unchanged and the tiling projection stays valid.
ALTER TABLE notes ADD COLUMN canvas_x REAL;
ALTER TABLE notes ADD COLUMN canvas_y REAL;
