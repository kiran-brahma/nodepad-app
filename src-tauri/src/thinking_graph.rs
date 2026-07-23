//! The Thinking Graph: the meaningful associations among Notes in one Thinking
//! Workspace, independent of how they are displayed.
//!
//! Everything here is a pure rule over committed state. Canonical ordering,
//! related lookup, degree, and pair validation live in this one place, so no
//! caller has to know that a symmetric pair is stored with its endpoints
//! sorted.

use serde::Serialize;

use crate::workspace::Note;

/// Who created a Relationship. This slice writes only `Manual`; `Ai` is part of
/// the durable vocabulary now so a later assistance slice adds rows rather than
/// changing what an existing row means.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationshipProvenance {
    Manual,
    Ai,
}

impl RelationshipProvenance {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Ai => "ai",
        }
    }

    pub(crate) fn from_str(value: &str) -> Self {
        match value {
            "ai" => Self::Ai,
            _ => Self::Manual,
        }
    }
}

/// A symmetric, untyped association between two distinct Notes in one Thinking
/// Workspace. It carries no direction and no named relation: `note_id_a` sorts
/// before `note_id_b` only so one pair has one row.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Relationship {
    pub(crate) id: String,
    pub(crate) workspace_id: String,
    pub(crate) note_id_a: String,
    pub(crate) note_id_b: String,
    pub(crate) provenance: RelationshipProvenance,
    pub(crate) created_at: String,
}

impl Relationship {
    /// The endpoint that is not `note_id`, or nothing when this Relationship
    /// does not touch that Note. Related lookup and degree are this, repeated.
    pub(crate) fn other_endpoint(&self, note_id: &str) -> Option<&str> {
        if self.note_id_a == note_id {
            Some(&self.note_id_b)
        } else if self.note_id_b == note_id {
            Some(&self.note_id_a)
        } else {
            None
        }
    }
}

/// Why a proposed pair is not a Relationship the Thinking Graph can hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GraphViolation {
    SelfRelationship,
    CrossWorkspace,
    MissingNote,
}

/// The canonical ordering of one unordered pair, so either endpoint order names
/// the same row.
pub(crate) fn canonical_pair<'a>(left: &'a str, right: &'a str) -> (&'a str, &'a str) {
    if left <= right {
        (left, right)
    } else {
        (right, left)
    }
}

/// Whether these two Notes are already related, in either endpoint order.
pub(crate) fn is_related(relationships: &[Relationship], left: &str, right: &str) -> bool {
    relationships
        .iter()
        .any(|relationship| relationship.other_endpoint(left) == Some(right))
}

/// Validates a proposed pair against the committed Notes and returns the
/// Workspace both belong to. Every rejection is decided before any transaction
/// opens, so an invalid pair can never leave partial state behind.
pub(crate) fn relatable_workspace_id(
    notes: &[Note],
    left: &str,
    right: &str,
) -> Result<String, GraphViolation> {
    if left == right {
        return Err(GraphViolation::SelfRelationship);
    }
    let endpoint = |note_id: &str| {
        notes
            .iter()
            .find(|note| note.id() == note_id)
            .ok_or(GraphViolation::MissingNote)
    };
    let left = endpoint(left)?;
    let right = endpoint(right)?;
    if left.workspace_id() != right.workspace_id() {
        return Err(GraphViolation::CrossWorkspace);
    }
    Ok(left.workspace_id().to_owned())
}
