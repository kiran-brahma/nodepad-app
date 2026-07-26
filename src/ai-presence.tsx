import type { EnrichmentStatus } from "./enrichment-controller"

/**
 * How present AI is right now: `working` while a Note is being organized,
 * `quiet` at every other moment. Two states only, so the shimmer on a Note
 * and the top-bar indicator can never disagree about whether AI is busy.
 */
export type AiPresence = "quiet" | "working"

/**
 * The one derivation of presence from the Enrichment Workflow's status. The
 * controller is single-flight — it holds one Note's status at a time — so
 * "is AI organizing anything" and "is this Note being organized" are the same
 * question asked of the same value. Nothing else keeps a copy.
 */
export function aiPresence(status: EnrichmentStatus | undefined): AiPresence {
  if (!status) return "quiet"
  return status.kind === "debouncing" || status.kind === "in_flight" ? "working" : "quiet"
}

/**
 * The only in-view AI signal besides the per-Note shimmer: one quiet
 * indicator in the top bar. A Manual Workspace renders nothing at all, so
 * Manual truly means manual — no AI configuration and no AI presence.
 */
export function AiPresenceIndicator({
  enabled,
  status,
}: {
  /** Whether the active Workspace's Assistance Policy permits an AI call. */
  enabled: boolean
  status: EnrichmentStatus
}) {
  if (!enabled) return null
  const presence = aiPresence(status)
  return (
    <span className={`ai-presence ${presence}`} role="status" aria-live="polite">
      <span className="ai-presence-dot" aria-hidden="true" />
      {presence === "working" ? "AI · working" : "AI · quiet"}
    </span>
  )
}
