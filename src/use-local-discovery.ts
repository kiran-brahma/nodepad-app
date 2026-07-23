import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import { thinkingWorkspace, type DiscoveryState, type ThinkingWorkspace } from "./workspace-client"

/**
 * Keeps the transient state of local model discovery. The hook only fetches
 * when the active Workspace uses Local AI; switching to Manual or changing
 * Workspaces clears the list. A request counter drops responses that arrive
 * after the thinker moved on, so a stale refresh can never overwrite current
 * state.
 */
export function useLocalDiscovery(activeWorkspace: ThinkingWorkspace | undefined) {
  const [state, setState] = useState<DiscoveryState>({ kind: "idle" })
  const [query, setQuery] = useState("")
  const latestRequest = useRef(0)

  const refresh = useCallback(async () => {
    const requestId = ++latestRequest.current
    setState({ kind: "loading" })
    const outcome = await thinkingWorkspace.discoverLocalModels()
    if (requestId !== latestRequest.current) return
    if (outcome.status === "failed") {
      setState({ kind: "error", failure: outcome.failure })
    } else {
      setState({ kind: "ready", models: outcome.models })
    }
  }, [])

  useEffect(() => {
    if (activeWorkspace?.assistancePolicy !== "local_ai") {
      setState({ kind: "idle" })
      setQuery("")
      return
    }
    void refresh()
  }, [activeWorkspace?.id, activeWorkspace?.assistancePolicy, refresh])

  const filteredModels = useMemo(() => {
    if (state.kind !== "ready") return []
    if (query.trim() === "") return state.models
    const needle = query.toLowerCase()
    return state.models.filter((model) => model.toLowerCase().includes(needle))
  }, [state, query])

  const selectedMissing = useMemo(() => {
    if (state.kind !== "ready" || !activeWorkspace?.selectedModel) return false
    return !state.models.includes(activeWorkspace.selectedModel)
  }, [state, activeWorkspace?.selectedModel])

  return {
    policy: activeWorkspace?.assistancePolicy ?? "manual",
    state,
    query,
    setQuery,
    refresh,
    filteredModels,
    selectedMissing,
  }
}
