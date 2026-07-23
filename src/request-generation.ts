import { useCallback, useMemo, useRef } from "react"

/**
 * The rule both AI controllers share: a response only lands if nothing has
 * superseded the request that asked for it.
 *
 * Every attempt takes a generation number when it starts. A later attempt, a
 * Workspace switch, or an explicit cancel bumps the counter, and the earlier
 * response is dropped on arrival rather than being written over fresher
 * state. This is the runtime half of staleness; the durable half is the
 * request token the Rust side re-checks before it commits anything.
 */
export interface RequestGeneration {
  /** Starts an attempt and returns the generation it belongs to. */
  begin: () => number
  /** Whether a response from that generation may still be applied. */
  isCurrent: (generation: number) => boolean
  /** Abandons whatever is in flight without starting anything. */
  supersede: () => void
}

export function useRequestGeneration(): RequestGeneration {
  const generationRef = useRef(0)
  const begin = useCallback(() => {
    generationRef.current += 1
    return generationRef.current
  }, [])
  const isCurrent = useCallback(
    (generation: number) => generationRef.current === generation,
    [],
  )
  const supersede = useCallback(() => {
    generationRef.current += 1
  }, [])
  // One stable handle for the life of the controller: an effect that
  // abandons in-flight work must not re-run merely because its owner
  // re-rendered.
  return useMemo(
    () => ({ begin, isCurrent, supersede }),
    [begin, isCurrent, supersede],
  )
}

/** The message an unexpected rejection carries into a failed status. */
export function failureMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error)
}
