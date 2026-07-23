import { useCallback, useEffect, useRef, useState } from "react"
import { failureMessage, useRequestGeneration } from "./request-generation"
import { assistanceEnabled, type WorkspaceSnapshot } from "./workspace-client"
import {
  enrichNote,
  type EnrichmentCommandOutcome,
  type EnrichmentFailureCode,
  type ParsedEnrichmentResult,
  type RequestToken,
} from "./enrichment-contracts"

/** The debounce window for the Edit-Note enrichment. Matches the
 *  Rust-side constant in `enrichment.rs` so the runtime and the
 *  spec stay in lockstep. */
export const ENRICH_DEBOUNCE_MILLIS = 800

/** What the UI sees for one Note. The controller owns the timer,
 *  the request token, and the latest outcome; the UI only reads
 *  the state and calls `schedule`, `retry`, or `replace`. */
export type EnrichmentStatus =
  | { kind: "idle" }
  | { kind: "debouncing" }
  | { kind: "in_flight"; token: RequestToken }
  | { kind: "applied"; result: ParsedEnrichmentResult; snapshot: WorkspaceSnapshot; at: string }
  | {
      kind: "failed"
      reason: "stale" | "invalid_schema" | "provider" | "unavailable"
      message: string
      code?: EnrichmentFailureCode
    }
  | { kind: "cancelled" }
  /** Re-enrich and Replace needs explicit confirmation. The UI
   *  renders the dialog while this state holds and calls
   *  `confirmReplace` to commit or `cancelReplace` to back out. */
  | { kind: "replace_pending"; reason: string }

export interface EnrichmentController {
  /** The current status for the most recently scheduled Note. A Note
   *  that is not the most recent reads `idle` until the UI asks for
   *  it again. */
  status: EnrichmentStatus
  /** Schedules an enrichment for the given Note. Replaces any pending
   *  debounce or in-flight request for the same Note. */
  schedule: (noteId: string) => void
  retry: () => void
  /** Asks the controller to surface a Re-enrich and Replace
   *  confirmation. The UI renders the dialog while the status is
   *  `replace_pending` and calls `confirmReplace` to commit. */
  requestReplace: () => void
  confirmReplace: () => void
  cancelReplace: () => void
  cancel: () => void
  clear: () => void
  /** The id of the Note whose status is currently shown. */
  activeNoteId: string | null
}

interface ScheduleOptions {
  workspaceId: string
  /** The most recent committed snapshot. Used for revision, model, and
   *  policy inputs to the request token. */
  snapshot: WorkspaceSnapshot | null
  /** Whether the thinker has selected a model and a policy that
   *  permit AI assistance. False suppresses the controller entirely
   *  so Manual Workspaces never see an in-flight state. */
  enabled: boolean
}

/** Returns a stable controller for the active Workspace. The hook
 *  owns the debounce timer, the request token, and the latest
 *  outcome. */
export function useEnrichmentController(options: ScheduleOptions): EnrichmentController {
  const { workspaceId, snapshot, enabled } = options
  const [status, setStatus] = useState<EnrichmentStatus>({ kind: "idle" })
  const [activeNoteId, setActiveNoteId] = useState<string | null>(null)
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const activeNoteIdRef = useRef<string | null>(null)
  const attempts = useRequestGeneration()

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const buildToken = useCallback(
    (noteId: string): RequestToken | null => {
      if (!snapshot) return null
      const workspace = snapshot.workspaces.find((candidate) => candidate.id === workspaceId)
      if (!workspace) return null
      const note = snapshot.notes.find((candidate) => candidate.id === noteId)
      if (!note) return null
      if (!workspace.selectedModel) return null
      const revision = note.enrichmentRevision
      return {
        workspaceId,
        noteId,
        revision,
        policy: workspace.assistancePolicy,
        endpoint:
          workspace.assistancePolicy === "cloud_ai"
            ? "https://ollama.com"
            : "http://localhost:11434",
        model: workspace.selectedModel,
      }
    },
    [snapshot, workspaceId],
  )

  const runEnrichment = useCallback(
    async (noteId: string, force: boolean) => {
      const token = buildToken(noteId)
      if (!token) {
        setStatus({ kind: "idle" })
        return
      }
      const generation = attempts.begin()
      setStatus({ kind: "in_flight", token })
      let outcome: EnrichmentCommandOutcome
      try {
        outcome = await enrichNote(workspaceId, noteId, force)
      } catch (error) {
        if (!attempts.isCurrent(generation)) return
        setStatus({ kind: "failed", reason: "unavailable", message: failureMessage(error) })
        return
      }
      if (!attempts.isCurrent(generation)) return
      applyOutcome(outcome, setStatus)
    },
    [attempts, buildToken, workspaceId],
  )

  const schedule = useCallback(
    (noteId: string) => {
      if (!enabled) {
        setStatus({ kind: "idle" })
        return
      }
      if (snapshot && !hasEligiblePolicy(snapshot, workspaceId)) {
        setStatus({ kind: "idle" })
        return
      }
      // Switching the active Note while a debounce is pending
      // re-points the timer to the new Note; the old Note's status
      // becomes idle.
      activeNoteIdRef.current = noteId
      setActiveNoteId(noteId)
      clearTimer()
      setStatus({ kind: "debouncing" })
      timerRef.current = setTimeout(() => {
        timerRef.current = null
        void runEnrichment(noteId, false)
      }, ENRICH_DEBOUNCE_MILLIS)
    },
    [clearTimer, enabled, runEnrichment, snapshot, workspaceId],
  )

  const retry = useCallback(() => {
    const noteId = activeNoteIdRef.current
    if (!noteId) return
    void runEnrichment(noteId, false)
  }, [runEnrichment])

  const requestReplace = useCallback(() => {
    // Re-enrich and Replace is the one explicit action that may
    // overwrite manual organization. Surface a confirmation before
    // the call so a tap does not silently destroy the thinker's
    // manual work; the UI renders the dialog while the status is
    // `replace_pending`.
    const noteId = activeNoteIdRef.current
    if (!noteId) return
    setStatus({
      kind: "replace_pending",
      reason:
        "Replace the manual Note Type, Annotation, and Labels with the AI suggestion. Continue?",
    })
  }, [])

  const confirmReplace = useCallback(() => {
    const noteId = activeNoteIdRef.current
    if (!noteId) return
    void runEnrichment(noteId, true)
  }, [runEnrichment])

  const cancelReplace = useCallback(() => {
    setStatus({ kind: "idle" })
  }, [])

  const cancel = useCallback(() => {
    clearTimer()
    attempts.supersede()
    setStatus({ kind: "cancelled" })
  }, [attempts, clearTimer])

  const clear = useCallback(() => {
    clearTimer()
    attempts.supersede()
    setStatus({ kind: "idle" })
  }, [attempts, clearTimer])

  useEffect(
    () => () => {
      clearTimer()
    },
    [clearTimer],
  )

  return {
    status,
    schedule,
    retry,
    requestReplace,
    confirmReplace,
    cancelReplace,
    cancel,
    clear,
    activeNoteId,
  }
}

function hasEligiblePolicy(snapshot: WorkspaceSnapshot, workspaceId: string): boolean {
  return assistanceEnabled(snapshot.workspaces.find((candidate) => candidate.id === workspaceId))
}

function applyOutcome(
  outcome: EnrichmentCommandOutcome,
  setStatus: (status: EnrichmentStatus) => void,
): void {
  switch (outcome.status) {
    case "applied":
      setStatus({
        kind: "applied",
        result: outcome.result,
        snapshot: outcome.snapshot,
        at: new Date().toISOString(),
      })
      // The status is durable on the Note (the "AI organized" badge
      // reads `lastEnrichedAt`), so the in-flight status can fade
      // back to idle without losing information.
      window.setTimeout(() => {
        setStatus({ kind: "idle" })
      }, 1500)
      return
    case "rejected":
      setStatus({
        kind: "failed",
        reason: "stale",
        message: outcome.reason,
      })
      return
    case "invalid_schema":
      setStatus({ kind: "failed", reason: "invalid_schema", message: outcome.reason })
      return
    case "provider_failed":
      setStatus({
        kind: "failed",
        reason: "provider",
        code: outcome.code,
        message: outcome.message,
      })
      return
    case "unavailable":
      setStatus({ kind: "failed", reason: "unavailable", message: outcome.reason })
      return
  }
}
