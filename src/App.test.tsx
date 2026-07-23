import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { cleanup, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
// `vi.mock` is hoisted above this import, so App sees the fake interface.
import { App } from "./App"
import type { Note, NoteType, WorkspaceOutcome, WorkspaceSnapshot } from "./workspace-client"

/**
 * A stand-in for the Rust interface, so these tests exercise the real controls
 * through the DOM. Durable semantics are proved by the Rust conformance suite;
 * here the fake only has to commit and report the same shapes.
 */
const workspaceId = "workspace-1"
let snapshot: WorkspaceSnapshot
let history: WorkspaceSnapshot[]
let created = 0
let createdLabels = 0

function committed(): WorkspaceOutcome {
  const notes = [...snapshot.notes].sort(
    (left, right) =>
      Number(right.pinned) - Number(left.pinned) || left.createdAt.localeCompare(right.createdAt),
  )
  snapshot = { ...snapshot, notes, undoableCommands: history.length }
  return { status: "committed", snapshot }
}

/** Every mutation records the state it replaced, the way undo history does. */
function mutate(change: (notes: Note[]) => Note[]): WorkspaceOutcome {
  history.push(snapshot)
  snapshot = { ...snapshot, notes: change(snapshot.notes) }
  return committed()
}

function replace(noteId: string, fields: Partial<Note>) {
  return mutate((notes) =>
    notes.map((note) => (note.id === noteId ? { ...note, ...fields } : note)),
  )
}

vi.mock("@tauri-apps/api/core", () => ({
  invoke: (command: string, args: Record<string, unknown> = {}) => {
    switch (command) {
      case "get_workspace_snapshot":
        return Promise.resolve(committed())
      case "create_note": {
        created += 1
        const note: Note = {
          id: `note-${created}`,
          workspaceId,
          markdown: String(args.markdown),
          noteType: "general",
          noteTypeProvenance: "default",
          annotation: null,
          annotationProvenance: "default",
          createdAt: `2026-07-22T10:0${created}:00+00:00`,
          updatedAt: `2026-07-22T10:0${created}:00+00:00`,
          pinned: false,
          labels: [],
        }
        return Promise.resolve(mutate((notes) => [...notes, note]))
      }
      case "edit_note_text":
        return Promise.resolve(replace(String(args.noteId), { markdown: String(args.markdown) }))
      case "set_note_type":
        return Promise.resolve(
          replace(String(args.noteId), {
            noteType: args.noteType as NoteType,
            noteTypeProvenance: "manual",
          }),
        )
      case "set_note_annotation": {
        const annotation = String(args.annotation).trim()
        return Promise.resolve(
          replace(String(args.noteId), {
            annotation: annotation === "" ? null : annotation,
            annotationProvenance: "manual",
          }),
        )
      }
      case "set_note_pinned":
        return Promise.resolve(replace(String(args.noteId), { pinned: Boolean(args.pinned) }))
      case "delete_note":
        return Promise.resolve(mutate((notes) => notes.filter((note) => note.id !== args.noteId)))
      case "attach_label": {
        const name = String(args.name).trim()
        const existing = snapshot.notes.flatMap((note) => note.labels).find((label) => label.name.toLowerCase() === name.toLowerCase())
        const label = existing ?? { id: `label-${++createdLabels}`, workspaceId, name }
        return Promise.resolve(replace(String(args.noteId), { labels: [...snapshot.notes.find((note) => note.id === args.noteId)!.labels, label] }))
      }
      case "detach_label":
        return Promise.resolve(replace(String(args.noteId), { labels: snapshot.notes.find((note) => note.id === args.noteId)!.labels.filter((label) => label.id !== args.labelId) }))
      case "rename_label":
        return Promise.resolve(mutate((notes) => notes.map((note) => ({ ...note, labels: note.labels.map((label) => label.id === args.labelId ? { ...label, name: String(args.name).trim() } : label) }))))
      case "remove_label":
        return Promise.resolve(mutate((notes) => notes.map((note) => ({ ...note, labels: note.labels.filter((label) => label.id !== args.labelId) }))))
      case "search_notes": {
        const query = String(args.query).toLowerCase()
        return Promise.resolve({ status: "committed", results: snapshot.notes.filter((note) => `${note.markdown} ${note.annotation ?? ""} ${note.labels.map((label) => label.name).join(" ")}`.toLowerCase().includes(query)).map((note) => ({ noteId: note.id, snippet: note.markdown, noteType: note.noteType, labels: note.labels, rank: 0 })) })
      }
      case "undo_last_change": {
        const previous = history.pop()
        if (!previous) {
          return Promise.resolve({
            status: "failed",
            failure: { code: "nothing_to_undo", message: "There is nothing left to undo." },
          } satisfies WorkspaceOutcome)
        }
        snapshot = previous
        return Promise.resolve(committed())
      }
      default:
        return Promise.resolve(committed())
    }
  },
}))

beforeEach(() => {
  created = 0
  createdLabels = 0
  history = []
  snapshot = {
    workspaces: [
      {
        id: workspaceId,
        name: "Research",
        createdAt: "2026-07-22T09:00:00+00:00",
        updatedAt: "2026-07-22T09:00:00+00:00",
      },
    ],
    notes: [],
    activeWorkspaceId: workspaceId,
    undoableCommands: 0,
  }
})

afterEach(cleanup)

async function captureNote(user: ReturnType<typeof userEvent.setup>, markdown: string) {
  await user.click(screen.getByLabelText("New Note"))
  await user.paste(markdown)
  await user.click(screen.getByRole("button", { name: "Commit Note" }))
  await screen.findAllByRole("listitem")
}

function noteCards() {
  return screen.getAllByRole("listitem")
}

describe("manual Note controls", () => {
  it("renders committed Markdown without enabling raw HTML", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "# A durable thought\n\n<img src=x onerror=\"alert(1)\">")

    expect(screen.getByRole("heading", { name: "A durable thought" })).toBeDefined()
    expect(document.querySelector(".markdown img")).toBeNull()
    expect(document.querySelector(".markdown script")).toBeNull()
    expect(screen.getByText(/onerror/)).toBeDefined()
  })

  it("edits Note text and shows the committed text", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "First thought")

    await user.click(screen.getByRole("button", { name: "Edit Note" }))
    const editor = screen.getByLabelText("Note text")
    await user.clear(editor)
    await user.paste("Revised thought")
    await user.click(screen.getByRole("button", { name: "Save Note text" }))

    expect(await screen.findByText("Revised thought")).toBeDefined()
    expect(screen.queryByText("First thought")).toBeNull()
  })

  it("assigns a fixed Note Type from the picker", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Is this true?")

    await user.selectOptions(screen.getByLabelText("Note Type"), "question")
    // The badge, not the picker's own option, reports the committed Note Type.
    await waitFor(() =>
      expect(within(noteCards()[0]).getByText("Question", { selector: ".badge" })).toBeDefined(),
    )
  })

  it("adds, edits, and clears an Annotation without touching the Note text", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "A thought worth a footnote")

    await user.click(screen.getByRole("button", { name: "Add Annotation" }))
    await user.click(screen.getByLabelText("Annotation"))
    await user.paste("Where this came from")
    await user.click(screen.getByRole("button", { name: "Save Annotation" }))
    expect(await screen.findByText("Where this came from")).toBeDefined()
    expect(screen.getByText("A thought worth a footnote")).toBeDefined()

    await user.click(screen.getByRole("button", { name: "Edit Annotation" }))
    await user.clear(screen.getByLabelText("Annotation"))
    await user.click(screen.getByRole("button", { name: "Save Annotation" }))
    await waitFor(() => expect(screen.queryByText("Where this came from")).toBeNull())
    expect(screen.getByText("A thought worth a footnote")).toBeDefined()
  })

  it("refuses to commit an Annotation longer than the durable bound", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Bounded")

    await user.click(screen.getByRole("button", { name: "Add Annotation" }))
    await user.click(screen.getByLabelText("Annotation"))
    await user.paste("🧠".repeat(2001))
    expect(screen.getByRole("button", { name: "Save Annotation" }).getAttribute("disabled")).not.toBeNull()
  })

  it("pins a Note ahead of unpinned Notes and unpins it again", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Older thought")
    await captureNote(user, "Newer thought")

    const cards = noteCards()
    expect(cards[0].textContent).toContain("Older thought")
    await user.click(within(cards[1]).getByRole("button", { name: "Pin" }))

    await waitFor(() => expect(noteCards()[0].textContent).toContain("Newer thought"))
    expect(noteCards()[0].textContent).toContain("Pinned")
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Unpin" }))
    await waitFor(() => expect(noteCards()[0].textContent).toContain("Older thought"))
  })

  it("deletes a Note after confirmation and restores it with the keyboard undo", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Deletable thought")

    await user.click(screen.getByRole("button", { name: "Delete Note" }))
    const confirmation = screen.getByRole("alertdialog", { name: "Confirm delete Note" })
    expect(confirmation.textContent).toContain("Deletable thought")
    await user.click(within(confirmation).getByRole("button", { name: "Keep it" }))
    expect(screen.getByText("Deletable thought")).toBeDefined()

    await user.click(screen.getByRole("button", { name: "Delete Note" }))
    await user.click(
      within(screen.getByRole("alertdialog", { name: "Confirm delete Note" })).getByRole("button", {
        name: "Delete Note",
      }),
    )
    await waitFor(() => expect(screen.queryByText("Deletable thought")).toBeNull())

    await user.keyboard("{Meta>}z{/Meta}")
    expect(await screen.findByText("Deletable thought")).toBeDefined()
  })

  it("offers no undo when this session has committed nothing", async () => {
    render(<App />)
    const undo = await screen.findByRole("button", { name: "Undo" })
    expect(undo.getAttribute("disabled")).not.toBeNull()
  })

  it("adds a Label and searches only the committed Workspace projection", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "A thought to recover")
    await user.click(screen.getByRole("button", { name: "Add Label" }))
    await user.type(screen.getByLabelText("Label"), "Rêverie")
    await user.click(screen.getByRole("button", { name: "Save Label" }))
    expect(await screen.findByText("Rêverie")).toBeDefined()
    await user.type(screen.getByLabelText("Search this Thinking Workspace"), "Rêverie")
    await user.click(screen.getByRole("button", { name: "Search" }))
    expect(within(screen.getByLabelText("Search Notes")).getByText("A thought to recover")).toBeDefined()
  })
})
