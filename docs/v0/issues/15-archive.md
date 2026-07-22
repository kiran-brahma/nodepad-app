## Parent

Part of #1.

## What to build

Add the V0 versioned Nodepad archive workflow. Export one complete Thinking Workspace as deterministic JSON and import a fully validated archive into a fresh Workspace with collision-safe identity remapping. Validation completes before the transaction begins; import commits all durable data or nothing.

## Decisions

- New V0 format only; no browser archive compatibility.
- Root contains format identifier, integer version, exported timestamp, application version, and one Workspace payload.
- Payload contains Workspace name/Assistance Policy reset target, Notes, Note Types, manual/AI provenance, Labels and memberships, symmetric Relationships, pending Syntheses and source mappings, bounded Synthesis novelty history, and safe source metadata.
- Imported Assistance Policy always resets to Manual. Selected models and consent do not import.
- Exclude provider keys, selected keychain references, transient AI state, view state, selection, undo, database metadata, paths, and backups.
- Validate required fields, lengths, enums, unique source IDs, Label normalization, Relationship endpoints/pairs, Synthesis source IDs, and archive size before mutation.
- Assign fresh Workspace, Note, Label, Relationship, and Synthesis IDs; remap internal references transactionally.
- Name collisions append deterministic ` (2)`, ` (3)` suffixes.
- Use native open/save dialogs and atomic write semantics.

## Acceptance criteria

- [ ] Exported archive has explicit format/version and deterministic durable content.
- [ ] Round-trip import preserves all intended domain meaning with fresh identities.
- [ ] Imported Workspace is Manual regardless of exported Assistance Policy.
- [ ] Malformed JSON, unknown version, oversize input, invalid enum, duplicate IDs/pairs, broken references, and limit violations fail before mutation.
- [ ] Name and identity collisions cannot overwrite existing data.
- [ ] Import rollback leaves the database unchanged on injected failure.
- [ ] Secrets and transient state are absent from exported bytes.
- [ ] Canceling either dialog is a successful no-op.
- [ ] Imported data remains valid after restart and through every view.

## Testing decisions

- Round-trip fixtures cover Unicode, all Note Types, Labels, manual/AI provenance, Relationships, pending Syntheses, source mapping, empty Workspace, and large Notes within limits.
- Negative fixtures cover each validator and confirm database row counts/state remain unchanged.
- Scan archive bytes for sentinel key, paths, and transient markers.

## Blocked by

- #7
- #13

## Scope fence

Do not import legacy `.nodepad`, support partial import/merge, export multiple Workspaces, encrypt archives, or include local provider configuration.

## Delivery workflow

Run `prd-simplicity-audit`, use a fresh `implement` session, then `code-review`, scoped `fallow`, focused tests and repository gates, and one PR against `main`.
