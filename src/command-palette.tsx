import { Command } from "cmdk"
import { useEffect } from "react"

import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"
import { useModalFocus } from "./modal-focus"

/**
 * Command-K toggles the palette from any surface. It is not a text-editing
 * shortcut, so intercepting it globally never steals from a field. Kept here,
 * next to the palette, so the shortcut and the surface it opens change
 * together; App only supplies the stable state setter.
 */
export function useCommandPaletteShortcut(
  setOpen: (updater: (open: boolean) => boolean) => void,
): void {
  useEffect(() => {
    function onKeydown(event: KeyboardEvent) {
      if (event.key.toLowerCase() !== "k") return
      if (!event.metaKey || event.ctrlKey || event.shiftKey || event.altKey) return
      event.preventDefault()
      setOpen((open) => !open)
    }
    window.addEventListener("keydown", onKeydown)
    return () => window.removeEventListener("keydown", onKeydown)
  }, [setOpen])
}

export interface PaletteAction {
  id: string
  /** The text the thinker matches and selects on. */
  label: string
  /** An optional grouping heading, so the list reads in sections. */
  group?: string
  disabled?: boolean
  run: () => void
}

/**
 * The Command-K palette. It renders a list of actions and runs the one the
 * thinker selects; it owns no business rule. App decides which actions exist
 * and what each does, so the palette never learns about Workspaces or
 * assistance policy.
 *
 * App mounts it only while it is open, so Escape (registered above modals in
 * the escape priority), focus trap/restore, and a click outside all dismiss
 * it for the lifetime of the mount.
 */
export function CommandPalette({
  onClose,
  actions,
}: {
  onClose: () => void
  actions: PaletteAction[]
}) {
  const ref = useModalFocus<HTMLDivElement>(true)
  useEscape(onClose, ESCAPE_PRIORITY.palette)

  const grouped = new Map<string, PaletteAction[]>()
  for (const action of actions) {
    const key = action.group ?? "Commands"
    const list = grouped.get(key) ?? []
    list.push(action)
    grouped.set(key, list)
  }

  return (
    <div
      className="palette-overlay"
      onMouseDown={(event) => {
        if (event.target === event.currentTarget) onClose()
      }}
    >
      <div
        ref={ref}
        className="palette"
        role="dialog"
        aria-modal="true"
        aria-label="Command palette"
      >
        <Command label="Command palette">
          <Command.Input placeholder="Type a command…" />
          <Command.List>
            <Command.Empty>No matching command.</Command.Empty>
            {[...grouped.entries()].map(([heading, list]) => (
              <Command.Group key={heading} heading={heading}>
                {list.map((action) => (
                  <Command.Item
                    key={action.id}
                    value={action.label}
                    disabled={action.disabled}
                    onSelect={() => {
                      if (action.disabled) return
                      action.run()
                      onClose()
                    }}
                  >
                    {action.label}
                  </Command.Item>
                ))}
              </Command.Group>
            ))}
          </Command.List>
        </Command>
      </div>
    </div>
  )
}