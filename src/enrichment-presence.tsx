/** The top bar's one AI signal. It observes controller state and deliberately
 *  contains no configuration or request action. */
export function EnrichmentPresence({
  enabled,
  working,
}: {
  enabled: boolean
  working: boolean
}) {
  if (!enabled) return null
  return (
    <span className="enrichment-presence" role="status" aria-live="polite">
      AI · {working ? "working" : "quiet"}
    </span>
  )
}
