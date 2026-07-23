import type { ReactNode } from "react"
import type { Note } from "./workspace-client"
import { arrangementWeight, noteArrangement, tilingPages, type NoteArrangement } from "./note-views"

/**
 * One page of the tiling view, arranged by repeated halving. Layout decides
 * only where a Note appears; the card it places is the same card every view
 * uses, so no view can offer an action another one lacks.
 */
function TiledNotes({
  arrangement,
  card,
}: {
  arrangement: NoteArrangement
  card: (note: Note) => ReactNode
}) {
  if (arrangement.kind === "note") return <>{card(arrangement.note)}</>
  return (
    <div className={`split ${arrangement.direction}`}>
      <div className="split-side" style={{ flex: arrangementWeight(arrangement.first) }}>
        <TiledNotes arrangement={arrangement.first} card={card} />
      </div>
      <div className="split-side" style={{ flex: arrangementWeight(arrangement.second) }}>
        <TiledNotes arrangement={arrangement.second} card={card} />
      </div>
    </div>
  )
}

/** The visible Notes as tiled pages, each page a split of the same result set. */
export function TilingView({ notes, card }: { notes: Note[]; card: (note: Note) => ReactNode }) {
  return (
    <div className="tiling">
      {tilingPages(notes).map((page, index) => (
        <div
          className="tiling-page"
          key={index}
          role="group"
          aria-label={`Tiled Notes, page ${index + 1}`}
        >
          <TiledNotes arrangement={noteArrangement(page)!} card={card} />
        </div>
      ))}
    </div>
  )
}
