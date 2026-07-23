import type { FormEvent } from "react"

/**
 * Searching reads the Thinking Workspace; it commits nothing and changes no
 * Note. A search narrows the Notes both views show, so this surface reports
 * how much it narrowed and never renders a second copy of the Notes.
 */
export function SearchSection({
  query,
  searching,
  matchCount,
  noteCount,
  canSearch,
  onQueryChange,
  onSearch,
  onClear,
}: {
  query: string
  searching: boolean
  matchCount: number
  noteCount: number
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
      {searching && (
        <p role="status">
          {matchCount} of {noteCount} Notes match this search.
        </p>
      )}
    </section>
  )
}
