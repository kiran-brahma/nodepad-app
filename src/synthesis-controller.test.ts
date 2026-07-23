import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { act, renderHook } from "@testing-library/react"
import {
  SYNTHESIS_DEBOUNCE_MILLIS,
  useSynthesisController,
  type SynthesisStatus,
} from "./synthesis-controller"
import type { PendingSynthesis, WorkspaceSnapshot } from "./workspace-client"

const proposeSynthesis = vi.fn()

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => {
    if (command === "propose_synthesis") return proposeSynthesis(args)
    return Promise.reject(new Error(`unknown command ${command}`))
  },
}))

const snapshot: WorkspaceSnapshot = {
  workspaces: [
    {
      id: "w",
      name: "Test",
      assistancePolicy: "local_ai",
      selectedModel: "phi3:latest",
      cloudConsentAt: null,
      createdAt: "2026-07-22T10:00:00+00:00",
      updatedAt: "2026-07-22T10:00:00+00:00",
    },
  ],
  notes: [],
  relationships: [],
  pendingSyntheses: [],
  activeWorkspaceId: "w",
  undoableCommands: 0,
}

const pending: PendingSynthesis = {
  id: "s1",
  workspaceId: "w",
  text: "Reliability and speed pull the same team in different directions.",
  sourceNoteIds: ["n1", "n2"],
  labels: ["delivery tradeoffs"],
  model: "phi3:latest",
  policy: "local_ai",
  createdAt: "2026-07-23T10:00:00+00:00",
  stale: false,
}

let received: WorkspaceSnapshot[] = []

function controller(enabled = true, workspaceId = "w") {
  return renderHook(() =>
    useSynthesisController({
      workspaceId,
      enabled,
      onSnapshot: (next) => received.push(next),
    }),
  )
}

/** Runs the debounce out and lets the awaited command settle. */
async function settle() {
  await act(async () => {
    vi.advanceTimersByTime(SYNTHESIS_DEBOUNCE_MILLIS)
    await Promise.resolve()
    await Promise.resolve()
  })
}

beforeEach(() => {
  vi.useFakeTimers()
  proposeSynthesis.mockReset()
  received = []
})

afterEach(() => {
  vi.useRealTimers()
})

describe("the Synthesis controller", () => {
  it("never asks for a Synthesis in a Manual Workspace", async () => {
    const { result } = controller(false)
    act(() => result.current.schedule())
    await settle()
    expect(proposeSynthesis).not.toHaveBeenCalled()
    expect(result.current.status).toEqual<SynthesisStatus>({ kind: "idle" })
  })

  it("collapses repeated scheduling into one attempt after the quiet period", async () => {
    proposeSynthesis.mockResolvedValue({ status: "no_insight", snapshot })
    const { result } = controller()
    act(() => {
      result.current.schedule()
      result.current.schedule()
      result.current.schedule()
    })
    expect(proposeSynthesis).not.toHaveBeenCalled()
    await settle()
    expect(proposeSynthesis).toHaveBeenCalledTimes(1)
    expect(proposeSynthesis).toHaveBeenCalledWith({ workspaceId: "w" })
  })

  it("reports a proposed Synthesis and forwards the committed snapshot", async () => {
    const committed = { ...snapshot, pendingSyntheses: [pending] }
    proposeSynthesis.mockResolvedValue({
      status: "proposed",
      synthesis: pending,
      snapshot: committed,
    })
    const { result } = controller()
    act(() => result.current.schedule())
    await settle()
    expect(result.current.status).toEqual<SynthesisStatus>({
      kind: "proposed",
      synthesis: pending,
    })
    expect(received).toEqual([committed])
  })

  it("treats a no-insight result as a quiet success rather than a failure", async () => {
    proposeSynthesis.mockResolvedValue({ status: "no_insight", snapshot })
    const { result } = controller()
    act(() => result.current.schedule())
    await settle()
    expect(result.current.status).toEqual<SynthesisStatus>({ kind: "no_insight" })
  })

  it("reports an ineligible attempt with its own explanation", async () => {
    proposeSynthesis.mockResolvedValue({
      status: "ineligible",
      reason: "cooling",
      message: "Synthesis has run recently. It will look again shortly.",
      snapshot,
    })
    const { result } = controller()
    act(() => result.current.schedule())
    await settle()
    expect(result.current.status).toEqual<SynthesisStatus>({
      kind: "ineligible",
      reason: "cooling",
      message: "Synthesis has run recently. It will look again shortly.",
    })
  })

  it("reports a stale result as a failure that changed nothing", async () => {
    proposeSynthesis.mockResolvedValue({
      status: "stale",
      reason: "The Notes behind this Synthesis have changed.",
      snapshot,
    })
    const { result } = controller()
    act(() => result.current.schedule())
    await settle()
    expect(result.current.status).toMatchObject({ kind: "failed", reason: "stale" })
  })

  it("reports a provider failure with its typed code", async () => {
    proposeSynthesis.mockResolvedValue({
      status: "provider_failed",
      code: "timeout",
      message: "Provider call failed.",
      snapshot,
    })
    const { result } = controller()
    act(() => result.current.schedule())
    await settle()
    expect(result.current.status).toEqual<SynthesisStatus>({
      kind: "failed",
      reason: "provider",
      code: "timeout",
      message: "Provider call failed.",
    })
  })

  it("reports a malformed provider body without touching any Note", async () => {
    proposeSynthesis.mockResolvedValue({
      status: "invalid_schema",
      reason: "unknown field `confidence`",
      snapshot,
    })
    const { result } = controller()
    act(() => result.current.schedule())
    await settle()
    expect(result.current.status).toMatchObject({ kind: "failed", reason: "invalid_schema" })
    expect(received).toEqual([snapshot])
  })

  it("abandons a scheduled attempt when the Workspace changes", async () => {
    proposeSynthesis.mockResolvedValue({ status: "no_insight", snapshot })
    const { result, rerender } = renderHook(
      ({ workspaceId }: { workspaceId: string }) =>
        useSynthesisController({
          workspaceId,
          enabled: true,
          onSnapshot: (next) => received.push(next),
        }),
      { initialProps: { workspaceId: "w" } },
    )
    act(() => result.current.schedule())
    rerender({ workspaceId: "other" })
    await settle()
    expect(proposeSynthesis).not.toHaveBeenCalled()
    expect(result.current.status).toEqual<SynthesisStatus>({ kind: "idle" })
  })

  it("runs immediately when asked to skip the local quiet period", async () => {
    proposeSynthesis.mockResolvedValue({ status: "no_insight", snapshot })
    const { result } = controller()
    await act(async () => {
      result.current.attemptNow()
      await Promise.resolve()
    })
    expect(proposeSynthesis).toHaveBeenCalledTimes(1)
  })
})
