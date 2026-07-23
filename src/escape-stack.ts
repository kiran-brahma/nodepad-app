import { useEffect, useRef } from "react"

/**
 * Coordinated Escape. macOS Escape closes the topmost dismissible surface, so
 * a modal must win over a background note-focus lock, and an inline draft
 * must win over neither starving the other. Each dismissible surface owns
 * its own close; none knows about any other.
 *
 * Priority is explicit because React runs child effects before parent
 * effects, so mount order is not "what is on top." A higher priority is
 * closer to the thinker: the command palette registers highest, modals
 * next, inline drafts lower, and note-focus lowest so it only fires when
 * nothing else is open.
 */

interface Entry {
  id: number
  priority: number
  close: () => void
}

const stack: Entry[] = []
let nextId = 1

function topmost(): Entry | undefined {
  let top: Entry | undefined
  for (const entry of stack) {
    if (
      !top ||
      entry.priority > top.priority ||
      (entry.priority === top.priority && entry.id > top.id)
    ) {
      top = entry
    }
  }
  return top
}

function onKeyDown(event: KeyboardEvent) {
  if (event.key !== "Escape") return
  if (event.defaultPrevented) return
  const top = topmost()
  if (!top) return
  event.preventDefault()
  top.close()
}

if (typeof window !== "undefined") {
  window.addEventListener("keydown", onKeyDown)
}

/**
 * Registers `close` as a dismissible surface for the lifetime of the
 * component. `close` is read through a ref so a fresh callback each render
 * never reorders the stack.
 */
export function useEscape(close: () => void, priority = 1): void {
  const ref = useRef(close)
  ref.current = close
  useEffect(() => {
    const id = nextId++
    stack.push({ id, priority, close: () => ref.current() })
    return () => {
      const index = stack.findIndex((entry) => entry.id === id)
      if (index !== -1) stack.splice(index, 1)
    }
  }, [priority])
}

/** Priority conventions kept in one place so every surface agrees. */
export const ESCAPE_PRIORITY = {
  palette: 80,
  modal: 60,
  dialog: 40,
  focus: 0,
} as const