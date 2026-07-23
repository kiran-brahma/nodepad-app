//! The V0 Nodepad archive format.
//!
//! One Thinking Workspace exports as a deterministic JSON document and
//! imports again into a fresh Workspace with collision-safe identities. This
//! module owns the format and its validation; the durable layer owns the SQL
//! that extracts and commits the payload. Nothing here reads or writes SQLite,
//! and nothing in the durable layer knows the JSON shape.
//!
//! Validation completes before the import transaction begins: a malformed,
//! unknown-version, oversize, or referentially broken archive fails closed and
//! touches nothing. The envelope carries the format identifier, the integer
//! version, the exported moment, and the application version; the payload
//! carries durable domain content only. Secrets, transient state, view state,
//! selection, undo history, provider configuration, paths, and backups are
//! never written.

use serde::{Deserialize, Serialize};

use crate::thinking_graph::{canonical_pair, Relationship, RelationshipProvenance};
use crate::workspace::{
    validated_label_name, validated_markdown, validated_note_type, validated_workspace_name,
    AssistancePolicy, Note, PendingSynthesis, Provenance, ThinkingWorkspace,
    MAX_ANNOTATION_SCALARS,
};

/// The format identifier every V0 archive carries in its root.
pub(crate) const ARCHIVE_FORMAT: &str = "nodepad";
/// The only version this implementation reads or writes.
pub(crate) const ARCHIVE_VERSION: u32 = 0;
/// The largest archive accepted for import, checked before JSON parsing so a
/// pathologically large file cannot consume the parser. Durable content fits
/// well inside this; a larger input is a malformed one.
pub(crate) const MAX_ARCHIVE_BYTES: usize = 64 * 1024 * 1024;

/// Why an archive could not be imported. Every variant is decided before any
/// durable row is touched.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ArchiveError {
    /// The input exceeded `MAX_ARCHIVE_BYTES`.
    Oversize,
    /// The input was not valid JSON.
    MalformedJson,
    /// The root `format` field was missing or not `"nodepad"`.
    UnknownFormat,
    /// The root `version` field was missing or not `0`.
    UnknownVersion,
    /// A required field was missing or empty where a value is required.
    MissingField(&'static str),
    /// A field exceeded its durable length bound.
    InvalidLength(&'static str),
    /// A field held a value outside its fixed enum.
    InvalidEnum(&'static str),
    /// Two entities in the archive claimed the same source id.
    DuplicateSourceId(&'static str),
    /// A Relationship endpoint, Note Label, or Synthesis source did not name a
    /// known entity in the archive.
    BrokenReference(&'static str),
    /// A Relationship pair appeared more than once, in either endpoint order.
    DuplicateRelationshipPair,
    /// Two Labels canonicalized to the same Workspace-local identity.
    DuplicateLabelNormalization,
}

impl ArchiveError {
    pub(crate) fn message(&self) -> String {
        match self {
            Self::Oversize => "The archive is larger than Nodepad can import.".to_owned(),
            Self::MalformedJson => "The archive is not valid JSON.".to_owned(),
            Self::UnknownFormat => "The archive is not a Nodepad archive.".to_owned(),
            Self::UnknownVersion => "The archive version is not supported.".to_owned(),
            Self::MissingField(field) => {
                format!("The archive is missing a required field: {field}.")
            }
            Self::InvalidLength(field) => format!("An archive field exceeded its limit: {field}."),
            Self::InvalidEnum(field) => format!("An archive field held an unknown value: {field}."),
            Self::DuplicateSourceId(field) => format!("The archive repeats a {field} id."),
            Self::BrokenReference(field) => format!("The archive references a missing {field}."),
            Self::DuplicateRelationshipPair => {
                "The archive repeats a Relationship between the same two Notes.".to_owned()
            }
            Self::DuplicateLabelNormalization => "The archive repeats a Label by name.".to_owned(),
        }
    }
}

/// The durable slice one Thinking Workspace contributes to an archive, handed
/// to this module by the durable layer. It speaks domain types, not the JSON
/// shape, so the format stays owned here.
pub(crate) struct ExportData {
    pub(crate) workspace: ThinkingWorkspace,
    pub(crate) notes: Vec<Note>,
    pub(crate) labels: Vec<ExportLabel>,
    pub(crate) relationships: Vec<Relationship>,
    /// Pending Syntheses whose sources all still live in this Workspace. A
    /// Synthesis whose source was deleted or moved out is not archived,
    /// because its source mapping no longer resolves here.
    pub(crate) pending_syntheses: Vec<PendingSynthesis>,
    pub(crate) synthesis_history: Vec<ExportSynthesisHistory>,
}

pub(crate) struct ExportLabel {
    pub(crate) id: String,
    pub(crate) name: String,
}

pub(crate) struct ExportSynthesisHistory {
    pub(crate) text: String,
    pub(crate) created_at: String,
}

/// The serialized and deserialized V0 archive. Field order is the on-disk
/// order; arrays are sorted before serialization so two exports of the same
/// durable content produce the same bytes except for the exported timestamp.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Archive {
    pub(crate) format: String,
    pub(crate) version: u32,
    pub(crate) exported_at: String,
    pub(crate) application_version: String,
    pub(crate) workspace: ArchiveWorkspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveWorkspace {
    pub(crate) name: String,
    /// The Assistance Policy the Workspace held when exported. An imported
    /// Workspace is always Manual regardless of this value; the field records
    /// the original policy for the reader without importing it.
    pub(crate) assistance_policy: AssistancePolicy,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) notes: Vec<ArchiveNote>,
    pub(crate) labels: Vec<ArchiveLabel>,
    pub(crate) relationships: Vec<ArchiveRelationship>,
    pub(crate) pending_syntheses: Vec<ArchivePendingSynthesis>,
    pub(crate) synthesis_history: Vec<ArchiveSynthesisHistory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveNote {
    pub(crate) id: String,
    pub(crate) markdown: String,
    pub(crate) note_type: String,
    pub(crate) note_type_provenance: Provenance,
    pub(crate) annotation: Option<String>,
    pub(crate) annotation_provenance: Provenance,
    pub(crate) created_at: String,
    pub(crate) updated_at: String,
    pub(crate) pinned: bool,
    pub(crate) enrichment_revision: u64,
    pub(crate) last_enriched_at: Option<String>,
    pub(crate) label_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveLabel {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveRelationship {
    pub(crate) id: String,
    pub(crate) note_id_a: String,
    pub(crate) note_id_b: String,
    pub(crate) provenance: RelationshipProvenance,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchivePendingSynthesis {
    pub(crate) id: String,
    pub(crate) text: String,
    pub(crate) source_note_ids: Vec<String>,
    pub(crate) labels: Vec<String>,
    pub(crate) model: String,
    pub(crate) policy: AssistancePolicy,
    pub(crate) created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ArchiveSynthesisHistory {
    pub(crate) text: String,
    pub(crate) created_at: String,
}

/// Builds an archive from one Workspace's durable slice. Arrays are sorted
/// deterministically so the durable content is stable across exports; only the
/// envelope timestamp and application version vary. Pending Syntheses whose
/// sources have left the Workspace are dropped, because their source mapping
/// no longer resolves here and a broken reference is not archivable.
pub(crate) fn build_archive(
    data: &ExportData,
    application_version: &str,
    exported_at: &str,
) -> Archive {
    let workspace_id = &data.workspace.id;
    let note_ids: std::collections::HashSet<&str> =
        data.notes.iter().map(|note| note.id.as_str()).collect();

    let mut notes: Vec<ArchiveNote> = data
        .notes
        .iter()
        .map(|note| ArchiveNote {
            id: note.id.clone(),
            markdown: note.markdown.clone(),
            note_type: note.note_type.clone(),
            note_type_provenance: note.note_type_provenance,
            annotation: note.annotation.clone(),
            annotation_provenance: note.annotation_provenance,
            created_at: note.created_at.clone(),
            updated_at: note.updated_at.clone(),
            pinned: note.pinned,
            enrichment_revision: note.enrichment_revision,
            last_enriched_at: note.last_enriched_at.clone(),
            label_ids: note.labels.iter().map(|label| label.id.clone()).collect(),
        })
        .collect();
    notes.sort_by(|left, right| (&left.created_at, &left.id).cmp(&(&right.created_at, &right.id)));

    let mut labels: Vec<ArchiveLabel> = data
        .labels
        .iter()
        .map(|label| ArchiveLabel {
            id: label.id.clone(),
            name: label.name.clone(),
        })
        .collect();
    labels.sort_by(|left, right| (&left.name, &left.id).cmp(&(&right.name, &right.id)));

    let mut relationships: Vec<ArchiveRelationship> = data
        .relationships
        .iter()
        .map(|relationship| ArchiveRelationship {
            id: relationship.id.clone(),
            note_id_a: relationship.note_id_a.clone(),
            note_id_b: relationship.note_id_b.clone(),
            provenance: relationship.provenance,
            created_at: relationship.created_at.clone(),
        })
        .collect();
    relationships
        .sort_by(|left, right| (&left.created_at, &left.id).cmp(&(&right.created_at, &right.id)));

    // A pending Synthesis is archived only when every source still lives in
    // this Workspace. A source that was deleted or moved out leaves a
    // provisional insight whose mapping no longer resolves, so it is not
    // archivable; the rest of the Workspace is unaffected.
    let mut pending_syntheses: Vec<ArchivePendingSynthesis> = data
        .pending_syntheses
        .iter()
        .filter(|pending| pending.workspace_id == *workspace_id)
        .filter(|pending| {
            pending
                .source_note_ids
                .iter()
                .all(|source| note_ids.contains(source.as_str()))
        })
        .map(|pending| ArchivePendingSynthesis {
            id: pending.id.clone(),
            text: pending.text.clone(),
            source_note_ids: pending.source_note_ids.clone(),
            labels: pending.labels.clone(),
            model: pending.model.clone(),
            policy: pending.policy,
            created_at: pending.created_at.clone(),
        })
        .collect();
    pending_syntheses
        .sort_by(|left, right| (&left.created_at, &left.id).cmp(&(&right.created_at, &right.id)));

    let mut synthesis_history: Vec<ArchiveSynthesisHistory> = data
        .synthesis_history
        .iter()
        .map(|entry| ArchiveSynthesisHistory {
            text: entry.text.clone(),
            created_at: entry.created_at.clone(),
        })
        .collect();
    synthesis_history.sort_by(|left, right| {
        (&left.created_at, &left.text).cmp(&(&right.created_at, &right.text))
    });

    Archive {
        format: ARCHIVE_FORMAT.to_owned(),
        version: ARCHIVE_VERSION,
        exported_at: exported_at.to_owned(),
        application_version: application_version.to_owned(),
        workspace: ArchiveWorkspace {
            name: data.workspace.name.clone(),
            assistance_policy: data.workspace.assistance_policy,
            created_at: data.workspace.created_at.clone(),
            updated_at: data.workspace.updated_at.clone(),
            notes,
            labels,
            relationships,
            pending_syntheses,
            synthesis_history,
        },
    }
}

/// Serializes an archive as pretty JSON. Field order follows the struct
/// declarations and arrays are already sorted, so the durable bytes are
/// deterministic across exports of the same content.
pub(crate) fn serialize_archive(archive: &Archive) -> Result<String, ArchiveError> {
    serde_json::to_string_pretty(archive).map_err(|_| ArchiveError::MalformedJson)
}

/// Parses and fully validates an archive before any durable mutation. Every
/// failure is returned here; the caller never opens a transaction on the
/// failure path.
pub(crate) fn parse_and_validate(bytes: &[u8]) -> Result<Archive, ArchiveError> {
    if bytes.len() > MAX_ARCHIVE_BYTES {
        return Err(ArchiveError::Oversize);
    }
    let archive: Archive =
        serde_json::from_slice(bytes).map_err(|_| ArchiveError::MalformedJson)?;
    validate(&archive)?;
    Ok(archive)
}

fn validate(archive: &Archive) -> Result<(), ArchiveError> {
    if archive.format != ARCHIVE_FORMAT {
        return Err(ArchiveError::UnknownFormat);
    }
    if archive.version != ARCHIVE_VERSION {
        return Err(ArchiveError::UnknownVersion);
    }
    if archive.exported_at.trim().is_empty() {
        return Err(ArchiveError::MissingField("exportedAt"));
    }
    if archive.application_version.trim().is_empty() {
        return Err(ArchiveError::MissingField("applicationVersion"));
    }
    validate_workspace(&archive.workspace)
}

fn require_timestamp(value: &str, field: &'static str) -> Result<(), ArchiveError> {
    if value.trim().is_empty() {
        Err(ArchiveError::MissingField(field))
    } else {
        Ok(())
    }
}

fn validate_workspace(workspace: &ArchiveWorkspace) -> Result<(), ArchiveError> {
    // The name is validated through the durable rule so the limit and the
    // archive's limit cannot drift apart.
    if validated_workspace_name(&workspace.name).is_err() {
        return Err(ArchiveError::InvalidLength("workspace name"));
    }
    require_timestamp(&workspace.created_at, "workspace createdAt")?;
    require_timestamp(&workspace.updated_at, "workspace updatedAt")?;
    // assistance_policy is a typed enum; serde already rejected an unknown
    // value, so no further enum check is needed here.

    let mut label_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut label_canonicals: std::collections::HashSet<String> = std::collections::HashSet::new();
    for label in &workspace.labels {
        if !label_ids.insert(label.id.as_str()) {
            return Err(ArchiveError::DuplicateSourceId("label"));
        }
        let (_, canonical) = validated_label_name(&label.name)
            .map_err(|_| ArchiveError::InvalidLength("label name"))?;
        if !label_canonicals.insert(canonical) {
            return Err(ArchiveError::DuplicateLabelNormalization);
        }
    }

    let mut note_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for note in &workspace.notes {
        if !note_ids.insert(note.id.as_str()) {
            return Err(ArchiveError::DuplicateSourceId("note"));
        }
        if validated_markdown(&note.markdown).is_err() {
            return Err(ArchiveError::MissingField("note markdown"));
        }
        if validated_note_type(&note.note_type).is_err() {
            return Err(ArchiveError::InvalidEnum("note type"));
        }
        if let Some(annotation) = &note.annotation {
            if annotation.chars().count() > MAX_ANNOTATION_SCALARS {
                return Err(ArchiveError::InvalidLength("annotation"));
            }
            if annotation.trim().is_empty() {
                return Err(ArchiveError::MissingField("note annotation"));
            }
        }
        require_timestamp(&note.created_at, "note createdAt")?;
        require_timestamp(&note.updated_at, "note updatedAt")?;
        for label_id in &note.label_ids {
            if !label_ids.contains(label_id.as_str()) {
                return Err(ArchiveError::BrokenReference("note label"));
            }
        }
    }

    let mut pairs: std::collections::HashSet<(String, String)> = std::collections::HashSet::new();
    for relationship in &workspace.relationships {
        if !note_ids.contains(relationship.note_id_a.as_str()) {
            return Err(ArchiveError::BrokenReference("relationship endpoint"));
        }
        if !note_ids.contains(relationship.note_id_b.as_str()) {
            return Err(ArchiveError::BrokenReference("relationship endpoint"));
        }
        if relationship.note_id_a == relationship.note_id_b {
            return Err(ArchiveError::BrokenReference("relationship endpoint"));
        }
        let (a, b) = canonical_pair(&relationship.note_id_a, &relationship.note_id_b);
        if !pairs.insert((a.to_owned(), b.to_owned())) {
            return Err(ArchiveError::DuplicateRelationshipPair);
        }
        require_timestamp(&relationship.created_at, "relationship createdAt")?;
    }

    let mut synthesis_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for synthesis in &workspace.pending_syntheses {
        if !synthesis_ids.insert(synthesis.id.as_str()) {
            return Err(ArchiveError::DuplicateSourceId("synthesis"));
        }
        if synthesis.text.trim().is_empty() {
            return Err(ArchiveError::MissingField("synthesis text"));
        }
        if synthesis.model.trim().is_empty() {
            return Err(ArchiveError::MissingField("synthesis model"));
        }
        require_timestamp(&synthesis.created_at, "synthesis createdAt")?;
        for source in &synthesis.source_note_ids {
            if !note_ids.contains(source.as_str()) {
                return Err(ArchiveError::BrokenReference("synthesis source"));
            }
        }
        for label in &synthesis.labels {
            if validated_label_name(label).is_err() {
                return Err(ArchiveError::InvalidLength("synthesis label"));
            }
        }
    }

    for history in &workspace.synthesis_history {
        if history.text.trim().is_empty() {
            return Err(ArchiveError::MissingField("synthesis history text"));
        }
        require_timestamp(&history.created_at, "synthesis history createdAt")?;
    }

    Ok(())
}

/// Appends a deterministic ` (2)`, ` (3)` suffix until the name does not collide
/// with an existing Workspace name. The first collision appends ` (2)`, the
/// next distinct number after that, so re-importing the same archive twice
/// produces stable, ordered names.
pub(crate) fn collision_safe_name(name: &str, existing: &[String]) -> String {
    let taken: std::collections::HashSet<&str> = existing.iter().map(String::as_str).collect();
    if !taken.contains(name) {
        return name.to_owned();
    }
    let mut counter = 2;
    loop {
        let candidate = format!("{name} ({counter})");
        if !taken.contains(candidate.as_str()) {
            return candidate;
        }
        counter += 1;
    }
}

/// The default filename for an exported Workspace, sanitized for the file
/// system while preserving safe Unicode characters. Mirrors the Markdown
/// export's stem rule so the two exports name files consistently.
pub(crate) fn default_filename(workspace_name: &str) -> String {
    let stem: String = workspace_name
        .chars()
        .map(|character| match character {
            '/' | ':' | '\\' | '\0' => '-',
            character if character.is_control() => ' ',
            character => character,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    format!(
        "{}.nodepad.json",
        if stem.is_empty() {
            "Thinking Workspace"
        } else {
            &stem
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn minimal_workspace_name() -> String {
        "Archive Workspace".to_owned()
    }

    fn empty_archive() -> Archive {
        Archive {
            format: ARCHIVE_FORMAT.to_owned(),
            version: ARCHIVE_VERSION,
            exported_at: "2026-07-23T12:00:00+00:00".to_owned(),
            application_version: "0.1.0".to_owned(),
            workspace: ArchiveWorkspace {
                name: minimal_workspace_name(),
                assistance_policy: AssistancePolicy::Manual,
                created_at: "2026-07-22T09:00:00+00:00".to_owned(),
                updated_at: "2026-07-22T09:00:00+00:00".to_owned(),
                notes: vec![],
                labels: vec![],
                relationships: vec![],
                pending_syntheses: vec![],
                synthesis_history: vec![],
            },
        }
    }

    #[test]
    fn a_valid_empty_archive_round_trips_through_json() {
        let archive = empty_archive();
        let json = serialize_archive(&archive).unwrap();
        let parsed = parse_and_validate(json.as_bytes()).unwrap();
        assert_eq!(parsed.workspace.name, "Archive Workspace");
        assert_eq!(parsed.version, ARCHIVE_VERSION);
        assert_eq!(parsed.format, ARCHIVE_FORMAT);
    }

    #[test]
    fn malformed_json_fails_before_any_validation() {
        let error = parse_and_validate(b"{ not json").unwrap_err();
        assert_eq!(error, ArchiveError::MalformedJson);
    }

    #[test]
    fn an_unknown_format_is_rejected() {
        let mut archive = empty_archive();
        archive.format = "other-app".to_owned();
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::UnknownFormat);
    }

    #[test]
    fn an_unknown_version_is_rejected() {
        let mut archive = empty_archive();
        archive.version = 1;
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::UnknownVersion);
    }

    #[test]
    fn an_oversize_input_is_rejected_before_parsing() {
        let oversize = vec![b' '; MAX_ARCHIVE_BYTES + 1];
        let error = parse_and_validate(&oversize).unwrap_err();
        assert_eq!(error, ArchiveError::Oversize);
    }

    #[test]
    fn an_invalid_note_type_is_rejected() {
        let mut archive = empty_archive();
        archive.workspace.notes.push(ArchiveNote {
            id: "n1".into(),
            markdown: "text".into(),
            note_type: "not-a-type".into(),
            note_type_provenance: Provenance::Default,
            annotation: None,
            annotation_provenance: Provenance::Default,
            created_at: "2026-07-22T09:00:00+00:00".into(),
            updated_at: "2026-07-22T09:00:00+00:00".into(),
            pinned: false,
            enrichment_revision: 0,
            last_enriched_at: None,
            label_ids: vec![],
        });
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::InvalidEnum("note type"));
    }

    #[test]
    fn a_broken_relationship_endpoint_is_rejected() {
        let mut archive = empty_archive();
        archive.workspace.relationships.push(ArchiveRelationship {
            id: "r1".into(),
            note_id_a: "missing".into(),
            note_id_b: "alsomissing".into(),
            provenance: RelationshipProvenance::Manual,
            created_at: "2026-07-22T09:00:00+00:00".into(),
        });
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(
            error,
            ArchiveError::BrokenReference("relationship endpoint")
        );
    }

    #[test]
    fn a_duplicate_relationship_pair_is_rejected_in_either_order() {
        let mut archive = empty_archive();
        archive.workspace.notes.push(ArchiveNote {
            id: "n1".into(),
            markdown: "a".into(),
            note_type: "general".into(),
            note_type_provenance: Provenance::Default,
            annotation: None,
            annotation_provenance: Provenance::Default,
            created_at: "2026-07-22T09:00:00+00:00".into(),
            updated_at: "2026-07-22T09:00:00+00:00".into(),
            pinned: false,
            enrichment_revision: 0,
            last_enriched_at: None,
            label_ids: vec![],
        });
        archive.workspace.notes.push(ArchiveNote {
            id: "n2".into(),
            markdown: "b".into(),
            note_type: "general".into(),
            note_type_provenance: Provenance::Default,
            annotation: None,
            annotation_provenance: Provenance::Default,
            created_at: "2026-07-22T09:01:00+00:00".into(),
            updated_at: "2026-07-22T09:01:00+00:00".into(),
            pinned: false,
            enrichment_revision: 0,
            last_enriched_at: None,
            label_ids: vec![],
        });
        archive.workspace.relationships.push(ArchiveRelationship {
            id: "r1".into(),
            note_id_a: "n1".into(),
            note_id_b: "n2".into(),
            provenance: RelationshipProvenance::Manual,
            created_at: "2026-07-22T09:02:00+00:00".into(),
        });
        // The same pair in the other endpoint order is still a duplicate.
        archive.workspace.relationships.push(ArchiveRelationship {
            id: "r2".into(),
            note_id_a: "n2".into(),
            note_id_b: "n1".into(),
            provenance: RelationshipProvenance::Manual,
            created_at: "2026-07-22T09:03:00+00:00".into(),
        });
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::DuplicateRelationshipPair);
    }

    #[test]
    fn a_note_label_referencing_an_unknown_label_is_rejected() {
        let mut archive = empty_archive();
        archive.workspace.notes.push(ArchiveNote {
            id: "n1".into(),
            markdown: "a".into(),
            note_type: "general".into(),
            note_type_provenance: Provenance::Default,
            annotation: None,
            annotation_provenance: Provenance::Default,
            created_at: "2026-07-22T09:00:00+00:00".into(),
            updated_at: "2026-07-22T09:00:00+00:00".into(),
            pinned: false,
            enrichment_revision: 0,
            last_enriched_at: None,
            label_ids: vec!["no-such-label".into()],
        });
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::BrokenReference("note label"));
    }

    #[test]
    fn two_labels_canonicalizing_to_the_same_name_are_rejected() {
        let mut archive = empty_archive();
        archive.workspace.labels.push(ArchiveLabel {
            id: "l1".into(),
            name: "Rivers".into(),
        });
        archive.workspace.labels.push(ArchiveLabel {
            id: "l2".into(),
            name: "rivers".into(),
        });
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::DuplicateLabelNormalization);
    }

    #[test]
    fn a_synthesis_source_pointing_at_a_missing_note_is_rejected() {
        let mut archive = empty_archive();
        archive
            .workspace
            .pending_syntheses
            .push(ArchivePendingSynthesis {
                id: "s1".into(),
                text: "An insight".into(),
                source_note_ids: vec!["missing".into()],
                labels: vec![],
                model: "llama3.1:latest".into(),
                policy: AssistancePolicy::LocalAi,
                created_at: "2026-07-22T09:00:00+00:00".into(),
            });
        let error =
            parse_and_validate(serialize_archive(&archive).unwrap().as_bytes()).unwrap_err();
        assert_eq!(error, ArchiveError::BrokenReference("synthesis source"));
    }

    #[test]
    fn collision_safe_name_is_unique_then_suffixed_deterministically() {
        let existing = vec!["Research".to_owned()];
        assert_eq!(collision_safe_name("Research", &existing), "Research (2)");
        let existing = vec!["Research".to_owned(), "Research (2)".to_owned()];
        assert_eq!(collision_safe_name("Research", &existing), "Research (3)");
        // A non-colliding name is returned unchanged.
        assert_eq!(collision_safe_name("Reading", &[]), "Reading");
    }
}
