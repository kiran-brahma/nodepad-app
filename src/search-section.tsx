import type { FormEvent } from "react"
import type { SearchResult } from "./workspace-client"
import { noteTypeLabel } from "./note-controls"

/** Searching reads the Thinking Workspace; it commits nothing and changes no Note. */
export function SearchSection({
  query,
  results,
  canSearch,
  onQueryChange,
  onSearch,
  onClear,
}: {
  query: string
  results: SearchResult[] | null
  canSearch: boolean
  onQueryChange: (query: string) => void
  onSearch: (event: FormEvent) => void
  onClear: () => void
}) {
  return (
    <section aria-label="Search Notes">
      <form onSubmit={onSearch}>
        <label htmlFor="search-notes">Search this Thinking Workspace</label>
        <input id="search-notes" value={query} onChange={(event) => onQueryChange(event.target.value)} placeholder="Search Notes, Annotations, or Labels" />
        <div className="row"><button type="submit" disabled={!canSearch}>Search</button><button type="button" onClick={onClear}>Clear search</button></div>
      </form>
      {results && <ul className="search-results">{results.map((result) => <li key={result.noteId}><span className="badge">{noteTypeLabel(result.noteType)}</span> {result.snippet} {result.labels.map((label) => <span className="badge" key={label.id}>{label.name}</span>)}</li>)}</ul>}
    </section>
  )
}
