import type { ReactNode } from "react"
import type { Note } from "./workspace-client"
import { arrangementWeight, noteArrangement, tilingPages, type NoteArrangement } from "./note-views"
import type { NoteFocus } from "./note-focus"

/**
 * One page of the tiling view, arranged by repeated halving. Layout decides
 * only where a Note appears; the card it places is the same card every view
 * uses, so no view can offer an action another one lacks.
 */
function TiledNotes({
  arrangement,
  focus,
  card,
}: {
  arrangement: NoteArrangement
  focus: NoteFocus
  card: (note: Note) => ReactNode
}) {
  if (arrangement.kind === "note") {
    const note = arrangement.note
    return (
      <div
        onMouseEnter={() => focus.hoverNote(note.id)}
        onMouseLeave={() => focus.hoverNote(null)}
        onFocus={() => focus.hoverNote(note.id)}
        onBlur={() => focus.hoverNote(null)}
      >
        {card(note)}
      </div>
    )
  }
  return (
    <div className={`split ${arrangement.direction}`}>
      <div className="split-side" style={{ flex: arrangementWeight(arrangement.first) }}>
        <TiledNotes arrangement={arrangement.first} focus={focus} card={card} />
      </div>
      <div className="split-side" style={{ flex: arrangementWeight(arrangement.second) }}>
        <TiledNotes arrangement={arrangement.second} focus={focus} card={card} />
      </div>
    </div>
  )
}

/** The visible Notes as tiled pages, each page a split of the same result set. */
export function TilingView({ notes, focus, card }: { notes: Note[]; focus: NoteFocus; card: (note: Note) => ReactNode }) {
  return (
    <div className="tiling">
      {tilingPages(notes).map((page, index) => (
        <div
          className="tiling-page"
          key={index}
          role="group"
          aria-label={`Tiled Notes, page ${index + 1}`}
        >
          <TiledNotes arrangement={noteArrangement(page)!} focus={focus} card={card} />
        </div>
      ))}
    </div>
  )
}
