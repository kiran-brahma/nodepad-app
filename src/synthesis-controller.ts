import { useCallback, useEffect, useRef, useState } from "react"
import type { PendingSynthesis, WorkspaceSnapshot } from "./workspace-client"
import type { EnrichmentFailureCode } from "./enrichment-contracts"
import {
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
  /** Schedules an attempt for the active Workspace. Repeated calls collapse
   *  into one; a Manual Workspace never schedules anything. */
  schedule: () => void
  /** Runs an attempt now, skipping the local quiet period. The durable
   *  eligibility rules still apply and may refuse it. */
  attemptNow: () => void
  clear: () => void
}

interface SynthesisOptions {
  workspaceId: string
  /** Whether the Workspace's Assistance Policy permits an AI call. False
   *  suppresses the controller entirely, so a Manual Workspace never
   *  requests a Synthesis. */
  enabled: boolean
  /** Receives the durable snapshot every attempt returns, so the rest of
   *  the app renders committed state rather than the controller's copy. */
  onSnapshot: (snapshot: WorkspaceSnapshot) => void
}

export function useSynthesisController(options: SynthesisOptions): SynthesisController {
  const { workspaceId, enabled, onSnapshot } = options
  const [status, setStatus] = useState<SynthesisStatus>({ kind: "idle" })
  const timerRef = useRef<ReturnType<typeof setTimeout> | null>(null)
  const generationRef = useRef(0)
  const snapshotRef = useRef(onSnapshot)
  snapshotRef.current = onSnapshot

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
    generationRef.current += 1
    const generation = generationRef.current
    setStatus({ kind: "in_flight" })
    let outcome: SynthesisCommandOutcome
    try {
      outcome = await proposeSynthesis(workspaceId)
    } catch (error) {
      if (generationRef.current !== generation) return
      setStatus({
        kind: "failed",
        reason: "unavailable",
        message: error instanceof Error ? error.message : String(error),
      })
      return
    }
    // A Workspace switch or a newer attempt supersedes this response.
    if (generationRef.current !== generation) return
    snapshotRef.current(outcome.snapshot)
    setStatus(statusFor(outcome))
  }, [enabled, workspaceId])

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

  const attemptNow = useCallback(() => {
    clearTimer()
    void run()
  }, [clearTimer, run])

  const clear = useCallback(() => {
    clearTimer()
    generationRef.current += 1
    setStatus({ kind: "idle" })
  }, [clearTimer])

  // A Workspace switch abandons any pending or in-flight attempt: the
  // request was assembled from the other Workspace's Notes.
  useEffect(() => {
    clearTimer()
    generationRef.current += 1
    setStatus({ kind: "idle" })
  }, [clearTimer, workspaceId])

  useEffect(
    () => () => {
      clearTimer()
    },
    [clearTimer],
  )

  return { status, schedule, attemptNow, clear }
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
