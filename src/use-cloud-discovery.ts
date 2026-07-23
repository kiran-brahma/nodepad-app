import { useCallback, useEffect, useMemo, useRef, useState } from "react"
import {
  thinkingWorkspace,
  type DiscoveryState,
  type ThinkingWorkspace,
} from "./workspace-client"

/**
 * Keeps the transient state of cloud model discovery. The hook only fetches
 * when the active Workspace is on Cloud AI, has consented, and reports a
 * bearer key in the keychain. Switching to Manual or Local, removing
 * consent, or removing the key clears the list. A request counter drops
 * responses that arrive after the thinker moved on, so a stale refresh can
 * never overwrite current state. The key itself is never read into state.
 */
export function useCloudDiscovery(activeWorkspace: ThinkingWorkspace | undefined) {
  const [state, setState] = useState<DiscoveryState>({ kind: "idle" })
  const [query, setQuery] = useState("")
  const [keyPresent, setKeyPresent] = useState(false)
  const latestRequest = useRef(0)

  const refreshKeyPresence = useCallback(async () => {
    const present = await thinkingWorkspace.cloudApiKeyPresent()
    setKeyPresent(present)
  }, [])

  const refresh = useCallback(async () => {
    if (!activeWorkspace) return
    const requestId = ++latestRequest.current
    setState({ kind: "loading" })
    const outcome = await thinkingWorkspace.discoverCloudModels(activeWorkspace.id)
    if (requestId !== latestRequest.current) return
    if (outcome.status === "failed") {
      setState({ kind: "error", failure: outcome.failure })
    } else {
      setState({ kind: "ready", models: outcome.models })
    }
  }, [activeWorkspace])

  useEffect(() => {
    if (activeWorkspace?.assistancePolicy !== "cloud_ai") {
      setState({ kind: "idle" })
      setQuery("")
      return
    }
    void refreshKeyPresence()
    if (activeWorkspace.cloudConsentAt) {
      void refresh()
    } else {
      setState({ kind: "idle" })
    }
  }, [activeWorkspace?.id, activeWorkspace?.assistancePolicy, activeWorkspace?.cloudConsentAt, refresh, refreshKeyPresence])

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
    state,
    query,
    setQuery,
    refresh,
    refreshKeyPresence,
    filteredModels,
    selectedMissing,
    keyPresent,
  }
}
