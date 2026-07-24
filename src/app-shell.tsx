import type { ReactNode } from "react"

/**
 * The three-region app shell: a left workspace rail, a main pane with a
 * scrollable content area and a pinned footer capture bar, and a reserved
 * top-bar region in the main pane for future slices (R3/R4/R6).
 *
 * This component owns only layout. It does not read state, dispatch commands,
 * or decide what goes in each region — that is App's job.
 */
export function AppShell({
  rail,
  main,
  footer,
}: {
  /** The left rail: workspace list and create form. */
  rail: ReactNode
  /** The main pane's scrollable content: committed Notes, assistance,
   *  search, synthesis, and the header. */
  main: ReactNode
  /** The footer capture bar: the note capture form and workspace controls. */
  footer: ReactNode
}) {
  return (
    <div className="app-shell">
      <nav className="app-rail" aria-label="Workspaces">
        {rail}
      </nav>
      <div className="app-main">
        <div className="app-main-topbar" />
        <div className="app-main-content">
          {main}
        </div>
        <div className="app-main-footer">
          {footer}
        </div>
      </div>
    </div>
  )
}
