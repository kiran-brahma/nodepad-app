import { useCallback, useEffect, useState } from "react"
import {
  thinkingWorkspace,
  type StorageOpenFailure,
  type WorkspaceFailure,
  type WorkspaceOutcome,
  type WorkspaceSnapshot,
} from "./workspace-client"

export interface WorkspaceState {
  snapshot: WorkspaceSnapshot | null
  openFailure: StorageOpenFailure | null
  failure: WorkspaceFailure | null
  /**
   * The one path a command takes into the view: a committed outcome replaces
   * the snapshot, a failed one is reported, and nothing is shown that the
   * Thinking Workspace has not already committed.
   */
  submit: (pending: Promise<WorkspaceOutcome>) => Promise<boolean>
  reportFailure: (failure: WorkspaceFailure) => void
  dismissFailure: () => void
}

export function useWorkspaceSnapshot(): WorkspaceState {
  const [snapshot, setSnapshot] = useState<WorkspaceSnapshot | null>(null)
  const [openFailure, setOpenFailure] = useState<StorageOpenFailure | null>(null)
  const [failure, setFailure] = useState<WorkspaceFailure | null>(null)

  const submit = useCallback(async (pending: Promise<WorkspaceOutcome>) => {
    const outcome = await pending
    if (outcome.status === "unavailable") {
      setOpenFailure(outcome.failure)
      return false
    }
    if (outcome.status === "failed") {
      setFailure(outcome.failure)
      return false
    }
    setSnapshot(outcome.snapshot)
    setOpenFailure(null)
    setFailure(null)
    return true
  }, [])

  useEffect(() => {
    void submit(thinkingWorkspace.getSnapshot())
  }, [submit])

  return {
    snapshot,
    openFailure,
    failure,
    submit,
    reportFailure: setFailure,
    dismissFailure: () => setFailure(null),
  }
}
