import type { ReactNode } from "react"

/**
 * The three-region app shell: a left workspace rail, a main pane with a
 * top bar, scrollable content area and a pinned footer capture bar.
 *
 * This component owns only layout. It does not read state, dispatch commands,
 * or decide what goes in each region — that is App's job.
 */
export function AppShell({
  rail,
  topbar,
  main,
  footer,
}: {
  /** The left rail: workspace list and create form. */
  rail: ReactNode
  /** The top bar: view switcher and other transient controls. */
  topbar: ReactNode
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
        <div className="app-main-topbar">
          {topbar}
        </div>
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
