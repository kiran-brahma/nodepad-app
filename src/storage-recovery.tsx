import { useEffect, useState } from "react"
import {
  thinkingWorkspace,
  type BackupRecord,
  type RestoreOutcome,
  type StorageOpenFailure,
  type WorkspaceSnapshot,
} from "./workspace-client"

const RECOVERY_HEADLINE: Record<StorageOpenFailure["category"], string> = {
  unreadable: "Nodepad could not read its local database.",
  migration: "Nodepad could not prepare its local database.",
  initialization: "Nodepad could not start its local storage.",
}

type RestoreFailureCode = Extract<RestoreOutcome, { status: "failed" }>["code"]

const RESTORE_FAILURE_HEADLINE: Record<RestoreFailureCode, string> = {
  not_found: "That backup no longer exists.",
  checksum_mismatch: "That backup's checksum does not match its manifest.",
  corrupt: "That backup is not a usable database.",
  unsupported_schema: "That backup is from a newer Nodepad this version cannot restore.",
  pre_restore_failed: "Nodepad could not make a safety copy before restoring.",
  replacement_failed: "Nodepad could not replace the local database.",
  reopen_failed: "Nodepad restored the backup but could not reopen it.",
  unavailable: "Nodepad could not reach its local data folder.",
}

/** Storage would not open, so nothing is shown that could be mistaken for lost
 *  thinking. The valid local backups are listed so the thinker can restore one
 *  without overwriting anything by hand. */
export function StorageRecovery({
  failure,
  onRetry,
  onQuit,
  onRestored,
}: {
  failure: StorageOpenFailure
  onRetry: () => void
  onQuit: () => void
  onRestored: (snapshot: WorkspaceSnapshot) => void
}) {
  const [backups, setBackups] = useState<BackupRecord[] | null>(null)
  const [restoringId, setRestoringId] = useState<string | null>(null)
  const [restoreFailure, setRestoreFailure] = useState<{ code: RestoreFailureCode; message: string } | null>(null)
  const [confirmingId, setConfirmingId] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false
    void thinkingWorkspace.listBackups().then((records) => {
      if (!cancelled) setBackups(records)
    })
    return () => {
      cancelled = true
    }
  }, [])

  function restore(backupId: string) {
    setConfirmingId(null)
    setRestoringId(backupId)
    setRestoreFailure(null)
    void thinkingWorkspace.restoreBackup(backupId).then((outcome: RestoreOutcome) => {
      setRestoringId(null)
      if (outcome.status === "restored") onRestored(outcome.snapshot)
      else setRestoreFailure({ code: outcome.code, message: outcome.message })
    })
  }

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

      <section className="recovery-backups" aria-label="Local backups">
        <h2>Local backups</h2>
        <p>
          These are verified backups in your application-data folder. Restoring one replaces the
          current database and reopens it; an invalid backup is never used.
        </p>
        {backups === null ? (
          <p className="muted">Looking for backups…</p>
        ) : backups.length === 0 ? (
          <p className="muted">No valid local backups were found.</p>
        ) : (
          <ul>
            {backups.map((backup) => (
              <li key={backup.id}>
                <div>
                  <strong>{new Date(backup.createdAt).toLocaleString()}</strong>
                  <span className="muted"> · {backup.kind.replace("_", " ")}</span>
                </div>
                <div className="muted">
                  schema {backup.schemaVersion} · Nodepad {backup.appVersion}
                </div>
                <div className="row">
                  {confirmingId === backup.id ? (
                    <>
                      <button
                        onClick={() => restore(backup.id)}
                        disabled={restoringId !== null}
                      >
                        Confirm restore
                      </button>
                      <button
                        onClick={() => setConfirmingId(null)}
                        disabled={restoringId !== null}
                      >
                        Cancel
                      </button>
                    </>
                  ) : (
                    <button
                      onClick={() => setConfirmingId(backup.id)}
                      disabled={restoringId !== null}
                    >
                      {restoringId === backup.id ? "Restoring…" : "Restore"}
                    </button>
                  )}
                </div>
              </li>
            ))}
          </ul>
        )}
        {restoreFailure && (
          <p role="alert">
            {RESTORE_FAILURE_HEADLINE[restoreFailure.code]} {restoreFailure.message}
          </p>
        )}
      </section>
    </main>
  )
}
