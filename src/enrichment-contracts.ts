import { invoke } from "@tauri-apps/api/core"
import type { NoteType, WorkspaceSnapshot } from "./workspace-client"

/** The fixed Note Type enum returned by the model. Order matches the
 *  approved Prompt A list and is part of the contract. The values
 *  are sourced from `workspace-client.ts` so the wire schema and the
 *  UI enum stay in lockstep. */
export type NoteTypeValue = NoteType
/** A request token identifies one enrichment attempt end-to-end. The
 *  Rust side rejects any response whose token does not match the
 *  current Note state, so an edit during inference invalidates the
 *  in-flight response. The values are produced once at request start
 *  and never mutated. */
export interface RequestToken {
  workspaceId: string
  noteId: string
  revision: number
  policy: "manual" | "local_ai" | "cloud_ai"
  endpoint: string
  model: string
}

export interface ParsedEnrichmentResult {
  noteType: NoteTypeValue
  labels: string[]
  annotation: string | null
  relatedNoteIds: string[]
}

/** The typed failures a single enrichment attempt can produce. The UI
 *  maps each code to a retry affordance. */
export type EnrichmentFailureCode =
  | "unavailable"
  | "timeout"
  | "unauthenticated"
  | "authentication_failed"
  | "rate_limited"
  | "missing_model"
  | "cancelled"
  | "malformed_response"

/** The result of an `enrich_note` Tauri command. One of five states:
 *  - `applied`: the result was parsed and committed
 *  - `rejected`: the result was parsed but the gate refused (manual,
 *    stale revision, or policy reverted)
 *  - `invalid_schema`: the provider's body did not match the contract
 *  - `provider_failed`: HTTP / network / auth / rate limit failure
 *  - `unavailable`: pre-flight refused (policy, model, or storage)
 *  The `snapshot` is the durable state the UI should render next. */
export type EnrichmentCommandOutcome =
  | { status: "applied"; result: ParsedEnrichmentResult; snapshot: WorkspaceSnapshot }
  | {
      status: "rejected"
      result: ParsedEnrichmentResult
      snapshot: WorkspaceSnapshot
      reason: string
    }
  | { status: "invalid_schema"; reason: string; snapshot: WorkspaceSnapshot }
  | {
      status: "provider_failed"
      code: EnrichmentFailureCode
      message: string
      snapshot: WorkspaceSnapshot
    }
  | { status: "unavailable"; reason: string; snapshot: WorkspaceSnapshot }

/** Calls the Rust enrichment command. The single Tauri surface for
 *  Note Organization. The controller is the only caller. */
export function enrichNote(
  workspaceId: string,
  noteId: string,
  force: boolean,
): Promise<EnrichmentCommandOutcome> {
  return invoke<EnrichmentCommandOutcome>("enrich_note", {
    workspaceId,
    noteId,
    force,
  })
}
