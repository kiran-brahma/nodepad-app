import { afterEach, describe, expect, it, vi } from "vitest"
import { cleanup, render, screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { SynthesisSection } from "./synthesis-section"
import type { Note, PendingSynthesis } from "./workspace-client"

function note(id: string, markdown: string): Note {
  return {
    id,
    workspaceId: "w",
    markdown,
    noteType: "claim",
    noteTypeProvenance: "manual",
    annotation: null,
    annotationProvenance: "default",
    createdAt: "2026-07-22T10:00:00+00:00",
    updatedAt: "2026-07-22T10:00:00+00:00",
    pinned: false,
    enrichmentRevision: 0,
    lastEnrichedAt: null,
    labels: [],
  }
}

function synthesis(overrides: Partial<PendingSynthesis> = {}): PendingSynthesis {
  return {
    id: "s1",
    workspaceId: "w",
    text: "Reliability and speed pull the same team in different directions.",
    sourceNoteIds: ["n1", "n2"],
    labels: ["delivery tradeoffs"],
    model: "phi3:latest",
    policy: "local_ai",
    createdAt: "2026-07-23T10:00:00+00:00",
    stale: false,
    ...overrides,
  }
}

const notes = [note("n1", "Ship faster this quarter"), note("n2", "Nothing may break")]

afterEach(cleanup)

describe("the provisional Synthesis panel", () => {
  it("shows a pending Synthesis with its sources and its AI provenance", () => {
    render(
      <SynthesisSection
        pending={[synthesis()]}
        notes={notes}
        status={{ kind: "idle" }}
        aiEnabled
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    )
    expect(screen.getByText(/Reliability and speed pull/)).toBeTruthy()
    expect(screen.getByText(/Suggested by phi3:latest/)).toBeTruthy()
    const sources = within(screen.getByLabelText("Source Notes"))
    expect(sources.getByText("Ship faster this quarter")).toBeTruthy()
    expect(sources.getByText("Nothing may break")).toBeTruthy()
    expect(within(screen.getByLabelText("Suggested Labels")).getByText("delivery tradeoffs")).toBeTruthy()
  })

  it("accepts and dismisses only when the thinker asks", async () => {
    const user = userEvent.setup()
    const onAccept = vi.fn()
    const onDismiss = vi.fn()
    render(
      <SynthesisSection
        pending={[synthesis()]}
        notes={notes}
        status={{ kind: "idle" }}
        aiEnabled
        onAccept={onAccept}
        onDismiss={onDismiss}
      />,
    )
    expect(onAccept).not.toHaveBeenCalled()
    await user.click(screen.getByRole("button", { name: "Accept as a Note" }))
    expect(onAccept).toHaveBeenCalledWith("s1")
    await user.click(screen.getByRole("button", { name: "Dismiss" }))
    expect(onDismiss).toHaveBeenCalledWith("s1")
  })

  it("refuses to accept a Synthesis whose sources have changed, but still dismisses it", async () => {
    const user = userEvent.setup()
    const onDismiss = vi.fn()
    render(
      <SynthesisSection
        pending={[synthesis({ stale: true })]}
        notes={notes}
        status={{ kind: "idle" }}
        aiEnabled
        onAccept={vi.fn()}
        onDismiss={onDismiss}
      />,
    )
    expect(
      screen.getByRole("button", { name: "Accept as a Note" }).hasAttribute("disabled"),
    ).toBe(true)
    expect(screen.getByText(/have changed/)).toBeTruthy()
    await user.click(screen.getByRole("button", { name: "Dismiss" }))
    expect(onDismiss).toHaveBeenCalledWith("s1")
  })

  it("names a source Note that is no longer here rather than dropping it", () => {
    render(
      <SynthesisSection
        pending={[synthesis({ stale: true, sourceNoteIds: ["n1", "gone"] })]}
        notes={notes}
        status={{ kind: "idle" }}
        aiEnabled
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    )
    expect(
      within(screen.getByLabelText("Source Notes")).getByText("A Note that is no longer here"),
    ).toBeTruthy()
  })

  it("reports no insight as an ordinary outcome, never as an alert", () => {
    render(
      <SynthesisSection
        pending={[]}
        notes={notes}
        status={{ kind: "no_insight" }}
        aiEnabled
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    )
    expect(screen.getByText(/Nothing worth proposing yet/)).toBeTruthy()
    expect(screen.queryAllByRole("alert")).toHaveLength(0)
  })

  it("explains an ineligible attempt in its own words", () => {
    render(
      <SynthesisSection
        pending={[]}
        notes={notes}
        status={{
          kind: "ineligible",
          reason: "too_few_organized_notes",
          message: "Synthesis needs at least five organized Notes in this Thinking Workspace.",
        }}
        aiEnabled
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    )
    expect(screen.getByText(/at least five organized Notes/)).toBeTruthy()
    expect(screen.queryAllByRole("alert")).toHaveLength(0)
  })

  it("offers no panel at all to a Manual Workspace with nothing pending", () => {
    const { container } = render(
      <SynthesisSection
        pending={[]}
        notes={notes}
        status={{ kind: "idle" }}
        aiEnabled={false}
        onAccept={vi.fn()}
        onDismiss={vi.fn()}
      />,
    )
    expect(container.firstChild).toBeNull()
  })
})
