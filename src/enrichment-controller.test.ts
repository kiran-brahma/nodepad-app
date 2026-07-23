import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { act, renderHook } from "@testing-library/react"
import {
  ENRICH_DEBOUNCE_MILLIS,
  useEnrichmentController,
  type EnrichmentStatus,
} from "./enrichment-controller"
import type { WorkspaceSnapshot } from "./workspace-client"

const enrichNote = vi.fn()
const debounce = ENRICH_DEBOUNCE_MILLIS

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args?: Record<string, unknown>) => {
    if (command === "enrich_note") return enrichNote(args)
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
  notes: [
    {
      id: "n",
      workspaceId: "w",
      markdown: "text",
      noteType: "general",
      noteTypeProvenance: "default",
      annotation: null,
      annotationProvenance: "default",
      createdAt: "2026-07-22T10:00:00+00:00",
      updatedAt: "2026-07-22T10:00:00+00:00",
      pinned: false,
      enrichmentRevision: 0,
      lastEnrichedAt: null,
      labels: [],
    },
  ],
  relationships: [],
  activeWorkspaceId: "w",
  undoableCommands: 0,
}

beforeEach(() => {
  vi.useFakeTimers()
  enrichNote.mockReset()
})

afterEach(() => {
  vi.useRealTimers()
})

describe("the enrichment controller", () => {
  it("is idle when the active Workspace is Manual", () => {
    const manual = {
      ...snapshot,
      workspaces: [{ ...snapshot.workspaces[0], assistancePolicy: "manual" as const }],
    }
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot: manual, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    expect(result.current.status.kind).toBe("idle")
  })

  it("debounces a scheduled call and only fires once", async () => {
    enrichNote.mockResolvedValue({
      status: "applied",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
      result.current.schedule("n")
      result.current.schedule("n")
    })
    expect(result.current.status.kind).toBe("debouncing")
    expect(enrichNote).not.toHaveBeenCalled()
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    expect(enrichNote).toHaveBeenCalledTimes(1)
  })

  it("replaces the active note when scheduling a different one mid-debounce", () => {
    const secondNote = {
      ...snapshot,
      notes: [
        snapshot.notes[0],
        {
          ...snapshot.notes[0],
          id: "m",
          workspaceId: "w",
        },
      ],
    }
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot: secondNote, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    act(() => {
      result.current.schedule("m")
    })
    expect(result.current.activeNoteId).toBe("m")
  })

  it("maps an applied response to a successful status", async () => {
    enrichNote.mockResolvedValue({
      status: "applied",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    expect(result.current.status.kind).toBe("applied")
  })

  it("maps a stale response to a failure with the stale reason", async () => {
    enrichNote.mockResolvedValue({
      status: "rejected",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
      reason: "The Note was edited during inference.",
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    const status: EnrichmentStatus = result.current.status
    expect(status.kind).toBe("failed")
    if (status.kind === "failed") {
      expect(status.reason).toBe("stale")
    }
  })

  it("maps a provider failure to a typed failure", async () => {
    enrichNote.mockResolvedValue({
      status: "provider_failed",
      code: "rate_limited",
      message: "Too many requests.",
      snapshot,
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    const status: EnrichmentStatus = result.current.status
    expect(status.kind).toBe("failed")
    if (status.kind === "failed") {
      expect(status.reason).toBe("provider")
      expect(status.code).toBe("rate_limited")
    }
  })

  it("cancels a pending debounce and discards the next response", async () => {
    let resolveInFlight: (value: unknown) => void = () => {}
    enrichNote.mockReturnValue(
      new Promise((resolve) => {
        resolveInFlight = resolve
      }),
    )
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    act(() => {
      result.current.cancel()
    })
    expect(result.current.status.kind).toBe("cancelled")
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    // Resolving the in-flight call after cancel must not overwrite the
    // cancelled state.
    await act(async () => {
      resolveInFlight({
        status: "applied",
        result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
        snapshot,
      })
      await Promise.resolve()
    })
    expect(result.current.status.kind).toBe("cancelled")
  })

  it("retry calls enrichNote with force = false", async () => {
    enrichNote.mockResolvedValue({
      status: "applied",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    enrichNote.mockClear()
    act(() => {
      result.current.retry()
    })
    await act(async () => {
      await Promise.resolve()
    })
    expect(enrichNote).toHaveBeenCalledWith({ workspaceId: "w", noteId: "n", force: false })
  })

  it("replace calls enrichNote with force = true after confirmation", async () => {
    enrichNote.mockResolvedValue({
      status: "applied",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    await act(async () => {
      await vi.advanceTimersByTimeAsync(debounce + 10)
    })
    enrichNote.mockClear()
    act(() => {
      result.current.requestReplace()
    })
    // requestReplace surfaces a confirmation; the actual enrich call
    // happens only after the thinker confirms.
    expect(enrichNote).not.toHaveBeenCalled()
    await act(async () => {
      await Promise.resolve()
    })
    act(() => {
      result.current.confirmReplace()
    })
    await act(async () => {
      await Promise.resolve()
    })
    expect(enrichNote).toHaveBeenCalledWith({ workspaceId: "w", noteId: "n", force: true })
  })

  it("requestReplace can be cancelled without calling enrich", async () => {
    enrichNote.mockResolvedValue({
      status: "applied",
      result: { noteType: "claim", labels: [], annotation: null, relatedNoteIds: [] },
      snapshot,
    })
    const { result } = renderHook(() =>
      useEnrichmentController({ workspaceId: "w", snapshot, enabled: true }),
    )
    act(() => {
      result.current.schedule("n")
    })
    act(() => {
      result.current.requestReplace()
    })
    expect(result.current.status.kind).toBe("replace_pending")
    act(() => {
      result.current.cancelReplace()
    })
    expect(result.current.status.kind).toBe("idle")
    expect(enrichNote).not.toHaveBeenCalled()
  })
})
