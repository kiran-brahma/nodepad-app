import { invoke } from "@tauri-apps/api/core"
import type { EnrichmentFailureCode } from "./enrichment-contracts"
import type { PendingSynthesis, WorkspaceOutcome, WorkspaceSnapshot } from "./workspace-client"

/** Why a Synthesis attempt did not run. Every value is an ordinary state
 *  the UI reports quietly; none of them is an error the thinker has to
 *  dismiss, and a Manual Workspace always reads `assistance_disabled`. */
export type IneligibleReason =
  | "assistance_disabled"
  | "too_few_organized_notes"
  | "too_little_diversity"
  | "too_few_new_notes"
  | "cooling"
  | "pending_cap_reached"

/** The result of one `propose_synthesis` attempt.
 *  - `proposed`: a valid, novel Synthesis is now pending — never a Note
 *  - `no_insight`: the attempt ran and found nothing worth proposing, or
 *    repeated an earlier Synthesis. A success, not a failure.
 *  - `ineligible`: the attempt did not run
 *  - `stale`: a source Note moved while the request was in flight
 *  - `invalid_schema` / `provider_failed` / `unavailable`: typed failures
 *  Every variant carries the snapshot the UI should render next. */
export type SynthesisCommandOutcome =
  | { status: "proposed"; synthesis: PendingSynthesis; snapshot: WorkspaceSnapshot }
  | { status: "no_insight"; snapshot: WorkspaceSnapshot }
  | {
      status: "ineligible"
      reason: IneligibleReason
      message: string
      snapshot: WorkspaceSnapshot
    }
  | { status: "stale"; reason: string; snapshot: WorkspaceSnapshot }
  | { status: "invalid_schema"; reason: string; snapshot: WorkspaceSnapshot }
  | {
      status: "provider_failed"
      code: EnrichmentFailureCode
      message: string
      snapshot: WorkspaceSnapshot
    }
  | { status: "unavailable"; reason: string; snapshot: WorkspaceSnapshot }

/** Asks the Rust side for one bounded Synthesis attempt. Eligibility, the
 *  cooldown clock, and the pending cap are decided there, against durable
 *  state, so a reload or a second window cannot bypass them. */
export function proposeSynthesis(workspaceId: string): Promise<SynthesisCommandOutcome> {
  return invoke<SynthesisCommandOutcome>("propose_synthesis", { workspaceId })
}

/** Accepts a pending Synthesis as a fresh thesis Note. */
export function acceptSynthesis(synthesisId: string): Promise<WorkspaceOutcome> {
  return invoke<WorkspaceOutcome>("accept_synthesis", { synthesisId })
}

/** Dismisses a pending Synthesis, keeping only its text for novelty. */
export function dismissSynthesis(synthesisId: string): Promise<WorkspaceOutcome> {
  return invoke<WorkspaceOutcome>("dismiss_synthesis", { synthesisId })
}
