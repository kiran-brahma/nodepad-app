import { useEffect, useRef } from "react"

const FOCUSABLE =
  "a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex='-1'])"

/**
 * Focus trap and restore-to-invoker for a true modal. On `open` it remembers
 * the element that had focus, moves focus into the dialog, and cycles Tab
 * within its focusable elements. On close it returns focus to the invoker so
 * a keyboard user lands back where they were instead of at the top of the
 * page.
 *
 * This is the only place that owns "a modal contains focus"; inline note
 * edits and the workspace rename form are not modals and keep their inline
 * style.
 */
export function useModalFocus<T extends HTMLElement>(open: boolean) {
  const ref = useRef<T>(null)

  useEffect(() => {
    if (!open) return
    const invoker = document.activeElement as HTMLElement | null
    const root = ref.current
    const first = root?.querySelector<HTMLElement>(FOCUSABLE)
    if (first) {
      first.focus()
    } else if (root) {
      root.setAttribute("tabindex", "-1")
      root.focus()
    }

    function onKeydown(event: KeyboardEvent) {
      if (event.key !== "Tab" || !ref.current) return
      const focusable = Array.from(
        ref.current.querySelectorAll<HTMLElement>(FOCUSABLE),
      )
      if (focusable.length === 0) return
      const first = focusable[0]
      const last = focusable[focusable.length - 1]
      const active = document.activeElement
      if (event.shiftKey && active === first) {
        event.preventDefault()
        last.focus()
      } else if (!event.shiftKey && active === last) {
        event.preventDefault()
        first.focus()
      }
    }

    document.addEventListener("keydown", onKeydown)
    return () => {
      document.removeEventListener("keydown", onKeydown)
      // Restore focus only if something inside the modal still holds it or
      // focus escaped to the body, never stealing focus from a control the
      // thinker has since moved to.
      const stillInside =
        root !== null &&
        document.activeElement !== null &&
        root.contains(document.activeElement)
      if (stillInside || document.activeElement === document.body) {
        invoker?.focus?.()
      }
    }
  }, [open])

  return ref
}