import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"
import { cleanup, render, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
// `vi.mock` is hoisted above this import, so App sees the fake interface.
import { App } from "./App"
import type {
  Note,
  NoteType,
  Relationship,
  WorkspaceOutcome,
  WorkspaceSnapshot,
} from "./workspace-client"

/**
 * A stand-in for the Rust interface, so these tests exercise the real controls
 * through the DOM. Durable semantics are proved by the Rust conformance suite;
 * here the fake only has to commit and report the same shapes.
 */
const workspaceId = "workspace-1"
/** A second Thinking Workspace, so a Note has somewhere to move or copy to. */
const otherWorkspaceId = "workspace-2"
let snapshot: WorkspaceSnapshot
let history: WorkspaceSnapshot[]
let created = 0
let createdLabels = 0
let createdRelationships = 0
let discoveryOutcome: import("./workspace-client").DiscoveryOutcome = {
  status: "committed",
  models: ["llama3.1:latest", "phi3:latest"],
}
let cloudDiscoveryOutcome: import("./workspace-client").DiscoveryOutcome = {
  status: "committed",
  models: ["qwen3:cloud", "gpt-oss:cloud"],
}
let cloudKeyPresent = false
let keychainWriteCount = 0
let lastSavedKey: string | null = null
let keychainDeleteCount = 0
let exportRequests = 0
let exportOutcome: import("./workspace-client").MarkdownExportOutcome = { status: "cancelled" }
let archiveExportRequests = 0
let archiveExportOutcome: import("./workspace-client").ArchiveExportOutcome = { status: "cancelled" }
let archiveImportRequests = 0
let archiveImportOutcome: import("./workspace-client").ArchiveImportOutcome = { status: "cancelled" }

/** The canonical pair the durable interface stores; order is not direction. */
function canonicalPair(left: string, right: string): [string, string] {
  return left <= right ? [left, right] : [right, left]
}

function committed(): WorkspaceOutcome {
  const notes = [...snapshot.notes].sort(
    (left, right) =>
      Number(right.pinned) - Number(left.pinned) || left.createdAt.localeCompare(right.createdAt),
  )
  snapshot = { ...snapshot, notes, undoableCommands: history.length }
  return { status: "committed", snapshot }
}

/**
 * Every mutation records the state it replaced, the way undo history does.
 * Deleting a Note takes its Relationships with it, as the schema's cascade
 * does.
 */
function mutate(change: (notes: Note[]) => Note[]): WorkspaceOutcome {
  history.push(snapshot)
  const notes = change(snapshot.notes)
  const surviving = new Set(notes.map((note) => note.id))
  snapshot = {
    ...snapshot,
    notes,
    relationships: snapshot.relationships.filter(
      (relationship) =>
        surviving.has(relationship.noteIdA) && surviving.has(relationship.noteIdB),
    ),
  }
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
          enrichmentRevision: 0,
          lastEnrichedAt: null,
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
      case "relate_notes": {
        const [noteIdA, noteIdB] = canonicalPair(String(args.noteId), String(args.otherNoteId))
        if (
          noteIdA === noteIdB ||
          snapshot.relationships.some(
            (relationship) =>
              relationship.noteIdA === noteIdA && relationship.noteIdB === noteIdB,
          )
        ) {
          return Promise.resolve(committed())
        }
        createdRelationships += 1
        const relationship: Relationship = {
          id: `relationship-${createdRelationships}`,
          workspaceId,
          noteIdA,
          noteIdB,
          provenance: "manual",
          createdAt: `2026-07-22T11:0${createdRelationships}:00+00:00`,
        }
        history.push(snapshot)
        snapshot = { ...snapshot, relationships: [...snapshot.relationships, relationship] }
        return Promise.resolve(committed())
      }
      case "unrelate_notes": {
        const [noteIdA, noteIdB] = canonicalPair(String(args.noteId), String(args.otherNoteId))
        history.push(snapshot)
        snapshot = {
          ...snapshot,
          relationships: snapshot.relationships.filter(
            (relationship) =>
              relationship.noteIdA !== noteIdA || relationship.noteIdB !== noteIdB,
          ),
        }
        return Promise.resolve(committed())
      }
      case "move_note": {
        // A move keeps identity and leaves every Relationship behind, because
        // a Relationship never crosses a Thinking Workspace.
        const moved = mutate((notes) =>
          notes.map((note) =>
            note.id === args.noteId
              ? { ...note, workspaceId: String(args.targetWorkspaceId) }
              : note,
          ),
        )
        snapshot = {
          ...snapshot,
          relationships: snapshot.relationships.filter(
            (relationship) =>
              relationship.noteIdA !== args.noteId && relationship.noteIdB !== args.noteId,
          ),
        }
        void moved
        return Promise.resolve(committed())
      }
      case "copy_note": {
        created += 1
        const source = snapshot.notes.find((note) => note.id === args.noteId)!
        const copy: Note = {
          ...source,
          id: `note-${created}`,
          workspaceId: String(args.targetWorkspaceId),
          createdAt: `2026-07-22T10:0${created}:00+00:00`,
          updatedAt: `2026-07-22T10:0${created}:00+00:00`,
        }
        return Promise.resolve(mutate((notes) => [...notes, copy]))
      }
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
      case "set_assistance_policy": {
        const policy = String(args.policy) as import("./workspace-client").AssistancePolicy
        history.push(snapshot)
        snapshot = {
          ...snapshot,
          workspaces: snapshot.workspaces.map((workspace) =>
            workspace.id === args.workspaceId
              ? { ...workspace, assistancePolicy: policy }
              : workspace,
          ),
        }
        return Promise.resolve(committed())
      }
      case "select_model": {
        const modelId = args.modelId as string | null
        history.push(snapshot)
        snapshot = {
          ...snapshot,
          workspaces: snapshot.workspaces.map((workspace) =>
            workspace.id === args.workspaceId
              ? { ...workspace, selectedModel: modelId }
              : workspace,
          ),
        }
        return Promise.resolve(committed())
      }
      case "discover_local_models":
        return Promise.resolve(discoveryOutcome)
      case "discover_cloud_models":
        return Promise.resolve(cloudDiscoveryOutcome)
      case "cloud_api_key_present":
        return Promise.resolve(cloudKeyPresent)
      case "set_cloud_api_key": {
        keychainWriteCount += 1
        lastSavedKey = String(args.apiKey)
        cloudKeyPresent = true
        return Promise.resolve({ status: "ok" } as const)
      }
      case "delete_cloud_api_key": {
        keychainDeleteCount += 1
        lastSavedKey = null
        cloudKeyPresent = false
        return Promise.resolve({ status: "ok" } as const)
      }
      case "set_cloud_consent": {
        const accept = Boolean(args.accept)
        history.push(snapshot)
        snapshot = {
          ...snapshot,
          workspaces: snapshot.workspaces.map((workspace) =>
            workspace.id === args.workspaceId
              ? {
                  ...workspace,
                  cloudConsentAt: accept
                    ? "2026-07-23T12:00:00+00:00"
                    : null,
                }
              : workspace,
          ),
        }
        return Promise.resolve(committed())
      }
      case "export_workspace":
        exportRequests += 1
        return Promise.resolve(exportOutcome)
      case "export_workspace_archive":
        archiveExportRequests += 1
        return Promise.resolve(archiveExportOutcome)
      case "import_workspace_archive":
        archiveImportRequests += 1
        return Promise.resolve(archiveImportOutcome)
      default:
        return Promise.resolve(committed())
    }
  },
}))

beforeEach(() => {
  created = 0
  createdLabels = 0
  createdRelationships = 0
  discoveryOutcome = {
    status: "committed",
    models: ["llama3.1:latest", "phi3:latest"],
  }
  cloudDiscoveryOutcome = {
    status: "committed",
    models: ["qwen3:cloud", "gpt-oss:cloud"],
  }
  cloudKeyPresent = false
  keychainWriteCount = 0
  lastSavedKey = null
  keychainDeleteCount = 0
  exportRequests = 0
  exportOutcome = { status: "cancelled" }
  archiveExportRequests = 0
  archiveExportOutcome = { status: "cancelled" }
  archiveImportRequests = 0
  archiveImportOutcome = { status: "cancelled" }
  history = []
  snapshot = {
    workspaces: [
      {
        id: workspaceId,
        name: "Research",
        assistancePolicy: "manual",
        selectedModel: null,
        cloudConsentAt: null,
        createdAt: "2026-07-22T09:00:00+00:00",
        updatedAt: "2026-07-22T09:00:00+00:00",
      },
      {
        id: otherWorkspaceId,
        name: "Reading",
        assistancePolicy: "manual",
        selectedModel: null,
        cloudConsentAt: null,
        createdAt: "2026-07-22T09:01:00+00:00",
        updatedAt: "2026-07-22T09:01:00+00:00",
      },
    ],
    notes: [],
    relationships: [],
    pendingSyntheses: [],
    activeWorkspaceId: workspaceId,
    undoableCommands: 0,
  }
})

afterEach(cleanup)

async function captureNote(user: ReturnType<typeof userEvent.setup>, markdown: string) {
  await user.click(screen.getByLabelText("New Note"))
  await user.paste(markdown)
  await user.click(screen.getByRole("button", { name: "Commit Note" }))
  await screen.findAllByRole("article")
}

function noteCards() {
  return screen.getAllByRole("article")
}

describe("manual Note controls", () => {
  it("exports the active Thinking Workspace through the native adapter", async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Export Markdown" }))

    await waitFor(() => expect(exportRequests).toBe(1))
  })

  it("exports a versioned archive and imports one into a fresh Workspace", async () => {
    archiveExportOutcome = { status: "exported", filename: "Research.nodepad.json" }
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Export Archive" }))
    await waitFor(() => expect(archiveExportRequests).toBe(1))

    // A successful import adopts a snapshot that adds one Workspace; the UI
    // surfaces it as a selectable Workspace alongside the existing ones.
    archiveImportOutcome = {
      status: "imported",
      snapshot: {
        ...snapshot,
        workspaces: [
          ...snapshot.workspaces,
          {
            id: "imported-workspace",
            name: "Research (2)",
            assistancePolicy: "manual",
            selectedModel: null,
            cloudConsentAt: null,
            createdAt: "2026-07-24T09:00:00+00:00",
            updatedAt: "2026-07-24T09:00:00+00:00",
          },
        ],
      },
    }
    await user.click(screen.getByRole("button", { name: "Import Archive" }))
    await waitFor(() => expect(archiveImportRequests).toBe(1))
    expect(await screen.findByRole("button", { name: "Research (2)" })).toBeDefined()
  })

  it("a cancelled import dialog is a successful no-op", async () => {
    archiveImportOutcome = { status: "cancelled" }
    const user = userEvent.setup()
    render(<App />)
    const before = archiveImportRequests
    await user.click(await screen.findByRole("button", { name: "Import Archive" }))
    await waitFor(() => expect(archiveImportRequests).toBe(before + 1))
    expect(screen.queryByRole("button", { name: /Research \(2\)/ })).toBeNull()
  })

  it("reports a failed archive import without adopting a snapshot", async () => {
    archiveImportOutcome = { status: "failed", message: "The archive is not valid JSON." }
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Import Archive" }))
    await waitFor(() => expect(archiveImportRequests).toBe(1))
    expect(await screen.findByText(/The archive is not valid JSON/)).toBeDefined()
  })

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
    await captureNote(user, "An unrelated thought")
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Add Label" }))
    await user.type(screen.getByLabelText("Label"), "Rêverie")
    await user.click(screen.getByRole("button", { name: "Save Label" }))
    expect(await screen.findByText("Rêverie")).toBeDefined()

    await user.type(screen.getByLabelText("Search this Thinking Workspace"), "Rêverie")
    await user.click(screen.getByRole("button", { name: "Search" }))

    // The search narrows the Notes on screen; it renders no second copy of them.
    await waitFor(() => expect(noteCards()).toHaveLength(1))
    expect(noteCards()[0].textContent).toContain("A thought to recover")
    expect(screen.queryByText("An unrelated thought")).toBeNull()
  })
})

describe("moving and copying a Note between Thinking Workspaces", () => {
  it("moves a Note out of this Workspace and restores it with undo", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "A thought that travels")

    await user.click(screen.getByRole("button", { name: "Move or Copy Note" }))
    // The destination is named, and the two outcomes are told apart in words.
    const chooser = screen.getByLabelText("Thinking Workspace to move or copy into")
    expect((chooser as HTMLSelectElement).value).toBe(otherWorkspaceId)
    expect(screen.getByText(/Move .* to Reading/)).toBeDefined()
    expect(screen.getByText(/loses its Relationships/)).toBeDefined()
    expect(screen.getByText(/This Note stays here/)).toBeDefined()

    await user.click(screen.getByRole("button", { name: "Move Note" }))
    await waitFor(() => expect(screen.queryByText("A thought that travels")).toBeNull())

    await user.keyboard("{Meta>}z{/Meta}")
    expect(await screen.findByText("A thought that travels")).toBeDefined()
  })

  it("copies a Note while the original stays in this Workspace", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "A thought worth repeating")

    await user.click(screen.getByRole("button", { name: "Move or Copy Note" }))
    await user.click(screen.getByRole("button", { name: "Copy Note" }))

    // The copy lives in the other Workspace, so this one still shows one Note.
    await waitFor(() => expect(noteCards()).toHaveLength(1))
    expect(screen.getByText("A thought worth repeating")).toBeDefined()
    expect(screen.queryByLabelText("Thinking Workspace to move or copy into")).toBeNull()
  })

  it("drops the Relationships a moved Note had", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Relate Note" }))
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Trade follows water" }))
    await waitFor(() => expect(within(noteCards()[0]).getByText("1 related")).toBeDefined())

    await user.click(within(noteCards()[1]).getByRole("button", { name: "Move or Copy Note" }))
    await user.click(within(noteCards()[1]).getByRole("button", { name: "Move Note" }))

    await waitFor(() => expect(noteCards()).toHaveLength(1))
    expect(within(noteCards()[0]).queryByText("1 related")).toBeNull()
  })

  it("offers no destination when this is the only Thinking Workspace", async () => {
    snapshot = { ...snapshot, workspaces: [snapshot.workspaces[0]] }
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Nowhere to go")

    expect(
      screen.getByRole("button", { name: "Move or Copy Note" }).getAttribute("disabled"),
    ).not.toBeNull()
  })
})

describe("Relationships on the Note detail surface", () => {
  /** Relates the first Note card to a Note matching `preview`. */
  async function relateFirstNote(
    user: ReturnType<typeof userEvent.setup>,
    query: string,
    preview: string,
  ) {
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Relate Note" }))
    await user.type(screen.getByLabelText("Relate to Note"), query)
    await user.click(within(noteCards()[0]).getByRole("button", { name: preview }))
  }

  it("adds a Relationship, lists it on either endpoint, and removes it", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")

    await relateFirstNote(user, "Trade", "Trade follows water")

    // Either endpoint lists the other, from one committed Relationship.
    await waitFor(() =>
      expect(
        within(within(noteCards()[0]).getByLabelText("Related Notes")).getByText(
          "Trade follows water",
        ),
      ).toBeDefined(),
    )
    expect(
      within(within(noteCards()[1]).getByLabelText("Related Notes")).getByText(
        "Cities grew around rivers",
      ),
    ).toBeDefined()
    expect(within(noteCards()[0]).getByText("1 related")).toBeDefined()

    await user.click(
      within(noteCards()[1]).getByRole("button", {
        name: "Remove Relationship to Cities grew around rivers",
      }),
    )
    await waitFor(() =>
      expect(
        within(within(noteCards()[0]).getByLabelText("Related Notes")).queryByText(
          "Trade follows water",
        ),
      ).toBeNull(),
    )
    expect(within(noteCards()[1]).queryByText("1 related")).toBeNull()
  })

  it("offers only Notes that are neither this one nor already related", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")

    await user.click(within(noteCards()[0]).getByRole("button", { name: "Relate Note" }))
    const editor = within(noteCards()[0])
    // A Note can never be offered itself.
    expect(editor.queryByRole("button", { name: "Cities grew around rivers" })).toBeNull()
    expect(editor.getByRole("button", { name: "Trade follows water" })).toBeDefined()
    // The search narrows the candidates to the Notes the thinker means.
    await user.type(screen.getByLabelText("Relate to Note"), "nothing matches this")
    expect(editor.queryByRole("button", { name: "Trade follows water" })).toBeNull()

    await user.clear(screen.getByLabelText("Relate to Note"))
    await user.click(editor.getByRole("button", { name: "Trade follows water" }))
    await waitFor(() => expect(within(noteCards()[0]).getByText("1 related")).toBeDefined())

    // An already-related Note is not offered a second time.
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Relate Note" }))
    expect(
      within(noteCards()[0]).queryByRole("button", { name: "Trade follows water" }),
    ).toBeNull()
  })

  it("navigates to a related Note without changing the Relationship", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")
    await relateFirstNote(user, "Trade", "Trade follows water")
    await waitFor(() => expect(within(noteCards()[0]).getByText("1 related")).toBeDefined())

    await user.click(
      within(noteCards()[0]).getByRole("button", { name: "Go to Trade follows water" }),
    )

    await waitFor(() => expect(noteCards()[1].getAttribute("aria-current")).toBe("true"))
    expect(document.activeElement).toBe(noteCards()[1])
    // Focus is transient UI state: the Relationship is untouched on both ends.
    expect(within(noteCards()[0]).getByText("1 related")).toBeDefined()
    expect(within(noteCards()[1]).getByText("1 related")).toBeDefined()
  })

  it("drops the Relationship when either Note is deleted", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")
    await relateFirstNote(user, "Trade", "Trade follows water")
    await waitFor(() => expect(within(noteCards()[0]).getByText("1 related")).toBeDefined())

    await user.click(within(noteCards()[1]).getByRole("button", { name: "Delete Note" }))
    await user.click(
      within(screen.getByRole("alertdialog", { name: "Confirm delete Note" })).getByRole("button", {
        name: "Delete Note",
      }),
    )

    await waitFor(() => expect(screen.queryByText("Trade follows water")).toBeNull())
    expect(within(noteCards()[0]).queryByText("1 related")).toBeNull()
    expect(
      within(within(noteCards()[0]).getByLabelText("Related Notes")).queryByRole("button", {
        name: /^Go to/,
      }),
    ).toBeNull()
  })
})

describe("tiling and kanban over one committed projection", () => {
  /** Switching view is a way of reading, so it submits nothing. */
  async function switchTo(
    user: ReturnType<typeof userEvent.setup>,
    view: "Tiling" | "Kanban" | "Graph",
  ) {
    await user.click(
      within(screen.getByRole("group", { name: "Note view" })).getByRole("button", { name: view }),
    )
  }

  it("shows the same Notes after a switch and commits nothing", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    const committed = snapshot

    await switchTo(user, "Kanban")
    expect(screen.getByText("Cities grew around rivers")).toBeDefined()
    await switchTo(user, "Tiling")
    expect(screen.getByText("Cities grew around rivers")).toBeDefined()
    expect(snapshot).toBe(committed)
  })

  it("edits a Note in one view and shows the committed text in the other", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "First thought")

    await switchTo(user, "Kanban")
    await user.click(screen.getByRole("button", { name: "Edit Note" }))
    await user.clear(screen.getByLabelText("Note text"))
    await user.paste("Revised in kanban")
    await user.click(screen.getByRole("button", { name: "Save Note text" }))
    expect(await screen.findByText("Revised in kanban")).toBeDefined()

    await switchTo(user, "Tiling")
    expect(screen.getByText("Revised in kanban")).toBeDefined()
    expect(screen.queryByText("First thought")).toBeNull()
  })

  it("groups Notes into one column per Note Type present", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Is this true?")
    await captureNote(user, "Rivers shaped trade")
    await user.selectOptions(within(noteCards()[0]).getByLabelText("Note Type"), "question")
    await waitFor(() =>
      expect(within(noteCards()[0]).getByText("Question", { selector: ".badge" })).toBeDefined(),
    )

    await switchTo(user, "Kanban")
    const question = screen.getByRole("group", { name: "Question Notes" })
    const general = screen.getByRole("group", { name: "General Notes" })
    expect(within(question).getAllByRole("article")).toHaveLength(1)
    expect(within(question).getByText("Is this true?")).toBeDefined()
    expect(within(general).getByText("Rivers shaped trade")).toBeDefined()
    // A Note Type nobody used has no column.
    expect(screen.queryByRole("group", { name: "Thesis Notes" })).toBeNull()
  })

  it("keeps the selected Note across a switch and lets it go when it leaves", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Relate Note" }))
    await user.click(within(noteCards()[0]).getByRole("button", { name: "Trade follows water" }))
    await user.click(
      within(noteCards()[0]).getByRole("button", { name: "Go to Trade follows water" }),
    )
    await waitFor(() => expect(noteCards()[1].getAttribute("aria-current")).toBe("true"))

    await switchTo(user, "Kanban")
    const selected = noteCards().find((card) => card.getAttribute("aria-current") === "true")
    expect(selected?.textContent).toContain("Trade follows water")

    await user.click(screen.getByLabelText("Search this Thinking Workspace"))
    await user.paste("Cities")
    await user.click(screen.getByRole("button", { name: "Search" }))

    // The selected Note is no longer on screen, so the selection is let go.
    await waitFor(() => expect(noteCards()).toHaveLength(1))
    expect(noteCards()[0].getAttribute("aria-current")).toBeNull()
  })

  it("filters both views from the same result set and keeps pinned Notes first", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Older thought about rivers")
    await captureNote(user, "Newer thought about rivers")
    await captureNote(user, "A thought about something else")
    await user.click(within(noteCards()[1]).getByRole("button", { name: "Pin" }))
    await waitFor(() => expect(noteCards()[0].textContent).toContain("Newer thought"))

    await user.click(screen.getByLabelText("Search this Thinking Workspace"))
    await user.paste("rivers")
    await user.click(screen.getByRole("button", { name: "Search" }))
    await waitFor(() => expect(noteCards()).toHaveLength(2))
    expect(noteCards()[0].textContent).toContain("Newer thought")
    expect(screen.getByRole("status").textContent).toContain("2 of 3 Notes match")

    await switchTo(user, "Kanban")
    expect(noteCards()).toHaveLength(2)
    expect(noteCards()[0].textContent).toContain("Newer thought")
    expect(screen.queryByText("A thought about something else")).toBeNull()
  })

  it("renders an empty, a one-Note, and a many-Note Workspace safely in both views", async () => {
    const user = userEvent.setup()
    render(<App />)
    expect(await screen.findByText("No Notes yet.")).toBeDefined()
    await switchTo(user, "Kanban")
    expect(screen.getByText("No Notes yet.")).toBeDefined()

    await switchTo(user, "Tiling")
    await captureNote(user, "The only thought")
    expect(noteCards()).toHaveLength(1)

    // More Notes than one tiled page holds.
    for (let index = 0; index < 8; index += 1) await captureNote(user, `Thought ${index}`)
    expect(noteCards()).toHaveLength(9)
    expect(screen.getAllByRole("group", { name: /Tiled Notes, page/ })).toHaveLength(2)
    await switchTo(user, "Kanban")
    expect(noteCards()).toHaveLength(9)
  })

  it("reconstructs both views from the committed snapshot after a restart", async () => {
    const user = userEvent.setup()
    render(<App />)
    await captureNote(user, "Older thought")
    await captureNote(user, "Newer thought")
    await user.click(within(noteCards()[1]).getByRole("button", { name: "Pin" }))
    await waitFor(() => expect(noteCards()[0].textContent).toContain("Newer thought"))

    // A restart keeps only what SQLite holds: no view choice, no layout.
    cleanup()
    render(<App />)
    await screen.findAllByRole("article")
    expect(noteCards()[0].textContent).toContain("Newer thought")
    expect(
      within(screen.getByRole("group", { name: "Note view" }))
        .getByRole("button", { name: "Tiling" })
        .getAttribute("aria-pressed"),
    ).toBe("true")

    await switchTo(user, "Kanban")
    expect(noteCards()[0].textContent).toContain("Newer thought")
  })
})

describe("the graph view and Relationship focus", () => {
  async function switchTo(
    user: ReturnType<typeof userEvent.setup>,
    view: "Tiling" | "Kanban" | "Graph",
  ) {
    await user.click(
      within(screen.getByRole("group", { name: "Note view" })).getByRole("button", { name: view }),
    )
  }

  function graph() {
    return screen.getByRole("group", { name: "Thinking Graph" })
  }

  /** One node per Note, found the same way a Note is found anywhere else. */
  function node(preview: string) {
    return within(graph()).getByRole("button", { name: preview })
  }

  /** No card is on screen while nothing is focused, so nothing is asserted into being. */
  function cards() {
    return screen.queryAllByRole("article")
  }

  function dimmedCards() {
    return cards().filter((card) => card.className.includes("dimmed"))
  }

  /** SVG class names are not plain strings, so the attribute is read directly. */
  function nodeClass(preview: string) {
    return node(preview).parentElement!.getAttribute("class") ?? ""
  }

  /** Two Notes related to each other, and a third related to neither. */
  async function threeNotes(user: ReturnType<typeof userEvent.setup>) {
    await captureNote(user, "Cities grew around rivers")
    await captureNote(user, "Trade follows water")
    await captureNote(user, "An unrelated thought")
    await user.click(within(cards()[0]).getByRole("button", { name: "Relate Note" }))
    await user.click(within(cards()[0]).getByRole("button", { name: "Trade follows water" }))
    await waitFor(() => expect(within(cards()[0]).getByText("1 related")).toBeDefined())
  }

  it("shows every Note of the Workspace once and every Relationship once", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")

    expect(within(graph()).getAllByRole("button")).toHaveLength(3)
    expect(node("Cities grew around rivers")).toBeDefined()
    expect(node("Trade follows water")).toBeDefined()
    // The unrelated Note is a node like any other, not a hidden one.
    expect(node("An unrelated thought")).toBeDefined()
    expect(graph().querySelectorAll("line")).toHaveLength(1)
  })

  it("previews related Notes on hover and lets them go again", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")

    await user.hover(node("Cities grew around rivers"))
    // Hover lights the Note and what it is related to; the rest is dimmed.
    await waitFor(() =>
      expect(nodeClass("An unrelated thought")).toContain("dimmed"),
    )
    expect(nodeClass("Trade follows water")).not.toContain("dimmed")

    await user.unhover(node("Cities grew around rivers"))
    await waitFor(() =>
      expect(nodeClass("An unrelated thought")).not.toContain("dimmed"),
    )
  })

  it("locks focus on click, opens the Note, and lets go on a second click", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")

    await user.click(node("Cities grew around rivers"))
    // Selecting a node opens the same Note card every other view places.
    await waitFor(() => expect(cards()).toHaveLength(1))
    expect(cards()[0].textContent).toContain("Cities grew around rivers")
    expect(node("Cities grew around rivers").getAttribute("aria-pressed")).toBe("true")
    expect(nodeClass("An unrelated thought")).toContain("dimmed")

    await user.click(node("Cities grew around rivers"))
    await waitFor(() => expect(cards()).toHaveLength(0))
    expect(node("Cities grew around rivers").getAttribute("aria-pressed")).toBe("false")
    // The pointer is still on the node, so its hover preview outlives the lock.
    await user.unhover(node("Cities grew around rivers"))
    await waitFor(() => expect(nodeClass("An unrelated thought")).not.toContain("dimmed"))
  })

  it("lets go of a locked focus on Escape", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")

    await user.click(node("Cities grew around rivers"))
    await waitFor(() => expect(cards()).toHaveLength(1))

    await user.keyboard("{Escape}")
    await waitFor(() => expect(cards()).toHaveLength(0))
    expect(node("Cities grew around rivers").getAttribute("aria-pressed")).toBe("false")
  })

  it("dims the same unrelated Notes in tiling and in kanban", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")
    await user.click(node("Cities grew around rivers"))
    await waitFor(() => expect(cards()).toHaveLength(1))

    await switchTo(user, "Tiling")
    await waitFor(() => expect(cards()).toHaveLength(3))
    const dimmedInTiling = dimmedCards().map((card) => card.getAttribute("aria-label"))
    expect(dimmedInTiling).toEqual(["An unrelated thought"])

    await switchTo(user, "Kanban")
    expect(dimmedCards().map((card) => card.getAttribute("aria-label"))).toEqual(dimmedInTiling)
  })

  it("lets go of the focus when the focused Note is deleted", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")
    await user.click(node("Trade follows water"))
    await waitFor(() => expect(cards()).toHaveLength(1))

    await user.click(within(cards()[0]).getByRole("button", { name: "Delete Note" }))
    await user.click(
      within(screen.getByRole("alertdialog", { name: "Confirm delete Note" })).getByRole("button", {
        name: "Delete Note",
      }),
    )

    // The node, its link, and the focus all go with the committed delete.
    await waitFor(() => expect(within(graph()).getAllByRole("button")).toHaveLength(2))
    expect(cards()).toHaveLength(0)
    expect(graph().querySelectorAll("line")).toHaveLength(0)
    expect(dimmedCards()).toHaveLength(0)
  })

  it("redraws from committed state when a Relationship is removed", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")
    expect(graph().querySelectorAll("line")).toHaveLength(1)

    await user.click(node("Cities grew around rivers"))
    await waitFor(() => expect(cards()).toHaveLength(1))
    await user.click(
      within(cards()[0]).getByRole("button", {
        name: "Remove Relationship to Trade follows water",
      }),
    )

    await waitFor(() => expect(graph().querySelectorAll("line")).toHaveLength(0))
    // Degree and the related set agree, because both read the one projection.
    expect(within(cards()[0]).queryByText("1 related")).toBeNull()
    expect(nodeClass("Trade follows water")).toContain("dimmed")
  })

  it("draws no link for a Relationship the Workspace no longer holds", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)

    // Moving a Note takes it and its Relationships out of this Workspace.
    await user.click(within(cards()[1]).getByRole("button", { name: "Move or Copy Note" }))
    await user.click(within(cards()[1]).getByRole("button", { name: "Move Note" }))
    await waitFor(() => expect(cards()).toHaveLength(2))

    await switchTo(user, "Graph")
    expect(within(graph()).getAllByRole("button")).toHaveLength(2)
    expect(graph().querySelectorAll("line")).toHaveLength(0)
  })

  it("draws an empty, a single-Note, and a disconnected Workspace safely", async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole("button", { name: "Undo" })
    await switchTo(user, "Graph")
    expect(screen.getByText("No Notes yet.")).toBeDefined()
    expect(screen.queryByRole("group", { name: "Thinking Graph" })).toBeNull()

    await switchTo(user, "Tiling")
    await captureNote(user, "The only thought")
    await switchTo(user, "Graph")
    expect(within(graph()).getAllByRole("button")).toHaveLength(1)
    expect(graph().querySelectorAll("line")).toHaveLength(0)

    // Every Note related to nothing is still a node, and none of them dim.
    await switchTo(user, "Tiling")
    for (let index = 0; index < 8; index += 1) await captureNote(user, `Thought ${index}`)
    await switchTo(user, "Graph")
    expect(within(graph()).getAllByRole("button")).toHaveLength(9)
    expect(graph().querySelectorAll("line")).toHaveLength(0)
    expect(nodeClass("Thought 0")).not.toContain("dimmed")
  })

  it("rebuilds the graph from the committed snapshot after a restart", async () => {
    const user = userEvent.setup()
    render(<App />)
    await threeNotes(user)
    await switchTo(user, "Graph")
    const drawn = graph().innerHTML

    // A restart keeps only what SQLite holds: no view choice, no arrangement.
    cleanup()
    render(<App />)
    await screen.findAllByRole("article")
    expect(
      within(screen.getByRole("group", { name: "Note view" }))
        .getByRole("button", { name: "Tiling" })
        .getAttribute("aria-pressed"),
    ).toBe("true")

    await switchTo(user, "Graph")
    expect(graph().innerHTML).toBe(drawn)
  })
})

describe("Assistance Policy and local Ollama discovery", () => {
  async function switchToLocalAi(user: ReturnType<typeof userEvent.setup>) {
    await user.click(await screen.findByText("Local AI"))
    await waitFor(() =>
      expect(
        screen.getByText("Local AI").closest("button")?.getAttribute("aria-pressed"),
      ).toBe("true"),
    )
  }

  it("starts with Manual assistance policy", async () => {
    render(<App />)
    expect(await screen.findByText(/Manual assistance/)).toBeDefined()
  })

  it("switches to Local AI and discovers models", async () => {
    const user = userEvent.setup()
    render(<App />)
    await switchToLocalAi(user)

    expect(await screen.findByText("llama3.1:latest")).toBeDefined()
    expect(screen.getByText("phi3:latest")).toBeDefined()
  })

  it("selects a model and returns to Manual", async () => {
    const user = userEvent.setup()
    render(<App />)
    await switchToLocalAi(user)

    const model = await screen.findByText("llama3.1:latest")
    await user.click(within(model.closest("li")!).getByRole("button", { name: "Select" }))

    await waitFor(() => expect(screen.getByText(/Local AI assistance/)).toBeDefined())
    expect(screen.getByText("llama3.1:latest").closest("li")?.textContent).toContain("Selected")

    await user.click(screen.getByRole("button", { name: "Manual" }))
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Manual" }).getAttribute("aria-pressed"),
      ).toBe("true"),
    )
  })

  it("searches the discovered model list", async () => {
    const user = userEvent.setup()
    render(<App />)
    await switchToLocalAi(user)
    await screen.findByText("phi3:latest")

    await user.type(screen.getByLabelText("Search models"), "llama")
    await waitFor(() => expect(screen.queryByText("phi3:latest")).toBeNull())
    expect(screen.getByText("llama3.1:latest")).toBeDefined()
  })

  it("reports a typed discovery failure", async () => {
    discoveryOutcome = {
      status: "failed",
      failure: { code: "unavailable", message: "Ollama is not running." },
    }
    const user = userEvent.setup()
    render(<App />)
    await switchToLocalAi(user)

    expect((await screen.findByRole("alert")).textContent).toContain("Ollama is not running.")
  })

  it("reports a missing selected model", async () => {
    snapshot = {
      ...snapshot,
      workspaces: snapshot.workspaces.map((workspace) =>
        workspace.id === workspaceId
          ? { ...workspace, assistancePolicy: "local_ai" as const, selectedModel: "retired-model:latest" }
          : workspace,
      ),
    }
    render(<App />)

    expect(
      await screen.findByText(/The selected model “retired-model:latest” is no longer available/),
    ).toBeDefined()
  })
})

describe("Ollama Cloud consent, keychain, and discovery", () => {
  /** A bearer key sentinel that the tests must never see in the snapshot. */
  const SENTINEL_KEY = "SENTINEL-BEARER-KEY-DO-NOT-LEAK"

  it("requires a disclosure before Cloud AI can be selected", async () => {
    const user = userEvent.setup()
    render(<App />)

    // Clicking Cloud AI on a non-consented Workspace opens the disclosure,
    // and the policy does not move to cloud_ai until the thinker accepts.
    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    expect(
      screen.getByText(/When this Thinking Workspace uses Cloud AI/),
    ).toBeDefined()
    expect(
      screen.getByRole("button", { name: "Cloud AI" }).getAttribute("aria-pressed"),
    ).not.toBe("true")
  })

  it("records consent and sets the policy only after the disclosure is accepted", async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    await user.click(
      screen.getByRole("button", { name: "I understand, enable Cloud AI" }),
    )

    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Cloud AI" }).getAttribute("aria-pressed"),
      ).toBe("true"),
    )
    expect(snapshot.workspaces[0].cloudConsentAt).not.toBeNull()
  })

  it("leaves the policy on its previous value when the disclosure is cancelled", async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    await user.click(screen.getByRole("button", { name: "Cancel" }))

    expect(snapshot.workspaces[0].assistancePolicy).toBe("manual")
    expect(snapshot.workspaces[0].cloudConsentAt).toBeNull()
  })

  it("saves a bearer key through the keychain and clears it on delete", async () => {
    cloudKeyPresent = true
    const user = userEvent.setup()
    render(<App />)

    // Bring the Workspace onto Cloud AI through the disclosure path so the
    // key section is on screen.
    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    await user.click(
      screen.getByRole("button", { name: "I understand, enable Cloud AI" }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Cloud AI" }).getAttribute("aria-pressed"),
      ).toBe("true"),
    )

    // The key input is shown; type a key and save.
    const input = (await screen.findByLabelText("Ollama Cloud key")) as HTMLInputElement
    await user.type(input, SENTINEL_KEY)
    await user.click(screen.getByRole("button", { name: "Save key" }))

    await waitFor(() => expect(keychainWriteCount).toBe(1))
    expect(lastSavedKey).toBe(SENTINEL_KEY)

    // The key value is gone from React state: the input is unmounted after save.
    expect(screen.queryByLabelText("Ollama Cloud key")).toBeNull()
    expect(screen.getByText("A key is saved in the macOS keychain.")).toBeDefined()

    // The snapshot never carries the bearer key.
    const serialized = JSON.stringify(snapshot)
    expect(serialized).not.toContain(SENTINEL_KEY)

    // Removing the key moves the Workspace back to the empty form.
    await user.click(screen.getByRole("button", { name: "Remove key" }))
    await waitFor(() => expect(keychainDeleteCount).toBe(1))
    expect(cloudKeyPresent).toBe(false)
  })

  it("discovers cloud models only when the Workspace is consented and a key is present", async () => {
    cloudKeyPresent = true
    snapshot = {
      ...snapshot,
      workspaces: snapshot.workspaces.map((workspace) =>
        workspace.id === workspaceId
          ? {
              ...workspace,
              assistancePolicy: "cloud_ai" as const,
              cloudConsentAt: "2026-07-23T12:00:00+00:00",
            }
          : workspace,
      ),
    }
    render(<App />)

    expect(await screen.findByText("qwen3:cloud")).toBeDefined()
    expect(screen.getByText("gpt-oss:cloud")).toBeDefined()
  })

  it("does not discover cloud models when the Workspace has no key", async () => {
    cloudKeyPresent = false
    snapshot = {
      ...snapshot,
      workspaces: snapshot.workspaces.map((workspace) =>
        workspace.id === workspaceId
          ? {
              ...workspace,
              assistancePolicy: "cloud_ai" as const,
              cloudConsentAt: "2026-07-23T12:00:00+00:00",
            }
          : workspace,
      ),
    }
    render(<App />)

    expect(await screen.findByText("Add your Ollama Cloud key to discover cloud-hosted models.")).toBeDefined()
    expect(screen.queryByText("qwen3:cloud")).toBeNull()
  })

  it("reports a missing selected cloud model", async () => {
    cloudKeyPresent = true
    snapshot = {
      ...snapshot,
      workspaces: snapshot.workspaces.map((workspace) =>
        workspace.id === workspaceId
          ? {
              ...workspace,
              assistancePolicy: "cloud_ai" as const,
              cloudConsentAt: "2026-07-23T12:00:00+00:00",
              selectedModel: "retired-cloud-model:tag",
            }
          : workspace,
      ),
    }
    render(<App />)

    expect(
      await screen.findByText(/The selected model “retired-cloud-model:tag” is no longer available/),
    ).toBeDefined()
  })

  it("reports a typed cloud discovery failure", async () => {
    cloudKeyPresent = true
    cloudDiscoveryOutcome = {
      status: "failed",
      failure: { code: "authentication_failed", message: "Ollama Cloud rejected the key." },
    }
    snapshot = {
      ...snapshot,
      workspaces: snapshot.workspaces.map((workspace) =>
        workspace.id === workspaceId
          ? {
              ...workspace,
              assistancePolicy: "cloud_ai" as const,
              cloudConsentAt: "2026-07-23T12:00:00+00:00",
            }
          : workspace,
      ),
    }
    render(<App />)

    expect((await screen.findByRole("alert")).textContent).toContain("Ollama Cloud rejected the key.")
  })

  it("isolates cloud consent between two Thinking Workspaces", async () => {
    const user = userEvent.setup()
    render(<App />)

    // Consent the active Workspace through the disclosure path.
    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    await user.click(
      screen.getByRole("button", { name: "I understand, enable Cloud AI" }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Cloud AI" }).getAttribute("aria-pressed"),
      ).toBe("true"),
    )

    // Switch to the second Workspace: it is not consented.
    await user.click(screen.getByRole("button", { name: "Reading" }))
    const reading = snapshot.workspaces.find((workspace) => workspace.name === "Reading")!
    expect(reading.cloudConsentAt).toBeNull()
    expect(reading.assistancePolicy).toBe("manual")
  })

  it("revoking cloud consent returns the Workspace to Manual", async () => {
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    await user.click(
      screen.getByRole("button", { name: "I understand, enable Cloud AI" }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Cloud AI" }).getAttribute("aria-pressed"),
      ).toBe("true"),
    )

    await user.click(
      screen.getByRole("button", { name: "Revoke Cloud consent for this Workspace" }),
    )
    await waitFor(() =>
      expect(
        screen.getByRole("button", { name: "Manual" }).getAttribute("aria-pressed"),
      ).toBe("true"),
    )
    const active = snapshot.workspaces.find((workspace) => workspace.id === snapshot.activeWorkspaceId)!
    expect(active.cloudConsentAt).toBeNull()
  })

  it("rejects a blank bearer key before it ever leaves the form", async () => {
    cloudKeyPresent = true
    const user = userEvent.setup()
    render(<App />)

    await user.click(await screen.findByRole("button", { name: "Cloud AI" }))
    await user.click(
      screen.getByRole("button", { name: "I understand, enable Cloud AI" }),
    )

    // The save button is disabled while the input is empty or whitespace.
    const saveButton = (await screen.findByRole("button", { name: "Save key" })) as HTMLButtonElement
    expect(saveButton.getAttribute("disabled")).not.toBeNull()
    await user.type(screen.getByLabelText("Ollama Cloud key"), "   ")
    expect(saveButton.getAttribute("disabled")).not.toBeNull()
    expect(keychainWriteCount).toBe(0)
  })
})
