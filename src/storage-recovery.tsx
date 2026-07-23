import type { StorageOpenFailure } from "./workspace-client"

const RECOVERY_HEADLINE: Record<StorageOpenFailure["category"], string> = {
  unreadable: "Nodepad could not read its local database.",
  migration: "Nodepad could not prepare its local database.",
  initialization: "Nodepad could not start its local storage.",
}

/** Storage would not open, so nothing is shown that could be mistaken for lost thinking. */
export function StorageRecovery({
  failure,
  onRetry,
  onQuit,
}: {
  failure: StorageOpenFailure
  onRetry: () => void
  onQuit: () => void
}) {
  return (
    <main>
      <header>
        <p className="eyebrow">Nodepad</p>
        <h1>Your thinking is still on disk</h1>
      </header>
      <section role="alert" className="recovery">
        <h2>{RECOVERY_HEADLINE[failure.category]}</h2>
        <p>{failure.message}</p>
        <p>
          Nothing has been reset or overwritten. Close anything else using this database, then try
          again.
        </p>
        <div className="row">
          <button onClick={onRetry}>Try again</button>
          <button onClick={onQuit}>Quit Nodepad</button>
        </div>
      </section>
    </main>
  )
}
