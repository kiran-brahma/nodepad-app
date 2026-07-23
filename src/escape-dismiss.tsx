import { useEscape, ESCAPE_PRIORITY } from "./escape-stack"

/**
 * Cancels its surface on Escape. Mounted only inside an open inline form, so
 * it registers nothing while the form is absent and never steals Escape from
 * the note-focus lock when nothing is being edited. One component, used by
 * every inline dismissible surface, so the Escape contract lives in one place.
 */
export function EscapeDismiss({ onEscape }: { onEscape: () => void }) {
  useEscape(onEscape, ESCAPE_PRIORITY.dialog)
  return null
}