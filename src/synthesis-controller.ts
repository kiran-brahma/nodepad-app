import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { failureMessage, useRequestGeneration } from "./request-generation"
import type { PendingSynthesis, WorkspaceSnapshot } from "./workspace-client"
import type { EnrichmentFailureCode } from "./enrichment-contracts"
import type { WorkspaceOutcome } from "./workspace-client"
import {
  acceptSynthesis,
  dismissSynthesis,
  proposeSynthesis,
  type IneligibleReason,
  type SynthesisCommandOutcome,
} from "./synthesis-contracts"

/** How long the controller waits after the Workspace changes before it asks
 *  for a Synthesis. This is only the local quiet period; the durable
 *  five-minute cooldown, the five-new-Notes checkpoint, and the pending cap
 *  all live in Rust, so a reload cannot shorten any of them. */
export const SYNTHESIS_DEBOUNCE_MILLIS = 2_000

/** What the UI shows for the Workspace's Synthesis attempts. `ineligible`
 *  and `no_insight` are quiet states, never alerts: not finding a Synthesis
 *  is a successful outcome. */
export type SynthesisStatus =
  | { kind: "idle" }
  | { kind: "debouncing" }
  | { kind: "in_flight" }
  | { kind: "proposed"; synthesis: PendingSynthesis }
  | { kind: "no_insight" }
  | { kind: "ineligible"; reason: IneligibleReason; message: string }
  | {
      kind: "failed"
      reason: "stale" | "invalid_schema" | "provider" | "unavailable"
      message: string
      code?: EnrichmentFailureCode
    }

export interface SynthesisController {
  status: SynthesisStatus
  /** The undecided Syntheses of this Workspace, read from committed state.
   *  One place answers "what is waiting on the thinker here", so the panel
   *  and the graph can never disagree about it. */
  pending: PendingSynthesis[]
  /** Schedules an attempt for the active Workspace. Repeated calls collapse
   *  into one; a Manual Workspace never schedules anything. */
  schedule: () => void
  /** Accepts one pending Synthesis as a fresh thesis Note. Only the thinker
   *  ever calls this; nothing accepts a Synthesis on their behalf. */
  accept: (synthesisId: string) => void
  /** Dismisses one pending Synthesis, keeping only its text for novelty. */
  dismiss: (synthesisId: string) => void
}

interface SynthesisOptions {
  workspaceId: string
  /** The most recent committed snapshot, which is where pending Syntheses
   *  are read from. */
  snapshot: WorkspaceSnapshot | null
  /** Whether the Workspace's Assistance Policy permits an AI call. False
   *  suppresses the controller entirely, so a Manual Workspace never
   *  requests a Synthesis. */
  enabled: boolean
  /** Receives the durable snapshot every attempt returns, so the rest of
   *  the app renders committed state rather than the controller's copy. */
  onSnapshot: (snapshot: WorkspaceSnapshot) => void
  /** The one path a command takes into the view. Accept and dismiss are
   *  ordinary Workspace commits and travel it like any other. */
  submit: (pending: Promise<WorkspaceOutcome>) => unknown
}

export function useSynthesisController(options: SynthesisOptions): SynthesisController {
  const { workspaceId, snapshot, enabled, onSnapshot, submit } = options
  const [status, setStatus] = useState<SynthesisStatus>({ kind: "idle" })
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const attempts = useRequestGeneration()
  const snapshotRef = useRef(onSnapshot)
  snapshotRef.current = onSnapshot
  const submitRef = useRef(submit)
  submitRef.current = submit

  const clearTimer = useCallback(() => {
    if (timerRef.current !== null) {
      clearTimeout(timerRef.current)
      timerRef.current = null
    }
  }, [])

  const run = useCallback(async () => {
    if (!enabled || workspaceId === "") {
      setStatus({ kind: "idle" })
      return
    }
    const generation = attempts.begin()
    setStatus({ kind: "in_flight" })
    let outcome: SynthesisCommandOutcome
    try {
      outcome = await proposeSynthesis(workspaceId)
    } catch (error) {
      if (!attempts.isCurrent(generation)) return
      setStatus({ kind: "failed", reason: "unavailable", message: failureMessage(error) })
      return
    }
    // A Workspace switch or a newer attempt supersedes this response.
    if (!attempts.isCurrent(generation)) return
    snapshotRef.current(outcome.snapshot)
    setStatus(statusFor(outcome))
  }, [attempts, enabled, workspaceId])

  const schedule = useCallback(() => {
    if (!enabled || workspaceId === "") {
      setStatus({ kind: "idle" })
      return
    }
    clearTimer()
    setStatus({ kind: "debouncing" })
    timerRef.current = setTimeout(() => {
      timerRef.current = null
      void run()
    }, SYNTHESIS_DEBOUNCE_MILLIS)
  }, [clearTimer, enabled, run, workspaceId])

  // A Workspace switch abandons any pending or in-flight attempt: the
  // request was assembled from the other Workspace's Notes.
  useEffect(() => {
    clearTimer()
    attempts.supersede()
    setStatus({ kind: "idle" })
  }, [attempts, clearTimer, workspaceId])

  useEffect(
    () => () => {
      clearTimer()
    },
    [clearTimer],
  )

  const accept = useCallback((synthesisId: string) => {
    submitRef.current(acceptSynthesis(synthesisId))
  }, [])

  const dismiss = useCallback((synthesisId: string) => {
    submitRef.current(dismissSynthesis(synthesisId))
  }, [])

  const pending = useMemo(
    () =>
      (snapshot?.pendingSyntheses ?? []).filter(
        (candidate) => candidate.workspaceId === workspaceId,
      ),
    [snapshot, workspaceId],
  )

  return { status, pending, schedule, accept, dismiss }
}

function statusFor(outcome: SynthesisCommandOutcome): SynthesisStatus {
  switch (outcome.status) {
    case "proposed":
      return { kind: "proposed", synthesis: outcome.synthesis }
    case "no_insight":
      return { kind: "no_insight" }
    case "ineligible":
      return { kind: "ineligible", reason: outcome.reason, message: outcome.message }
    case "stale":
      return { kind: "failed", reason: "stale", message: outcome.reason }
    case "invalid_schema":
      return { kind: "failed", reason: "invalid_schema", message: outcome.reason }
    case "provider_failed":
      return {
        kind: "failed",
        reason: "provider",
        code: outcome.code,
        message: outcome.message,
      }
    case "unavailable":
      return { kind: "failed", reason: "unavailable", message: outcome.reason }
  }
}
