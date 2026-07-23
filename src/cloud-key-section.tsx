import { FormEvent, useState } from "react"

import { thinkingWorkspace, type CloudSecretOutcome } from "./workspace-client"

/**
 * The Cloud AI key section. The thinker pastes a key here once, Nodepad
 * stores it in the macOS keychain through a narrow seam, and the key is
 * never read back. A second paste replaces the saved key; a button removes
 * it. The form is unmounted as soon as a save succeeds, so the key value
 * leaves React state immediately.
 */
export function CloudKeySection({
  keyPresent,
  onChange,
}: {
  keyPresent: boolean
  onChange: () => void
}) {
  const [editing, setEditing] = useState(!keyPresent)
  const [draft, setDraft] = useState("")
  const [submitting, setSubmitting] = useState(false)
  const [failure, setFailure] = useState<string | null>(null)

  async function saveKey(event: FormEvent) {
    event.preventDefault()
    if (draft.trim() === "") {
      setFailure("The Ollama Cloud key may not be blank.")
      return
    }
    setSubmitting(true)
    setFailure(null)
    const outcome: CloudSecretOutcome = await thinkingWorkspace.setCloudApiKey(draft)
    // Drop the value from React state as soon as the call resolves.
    setDraft("")
    setSubmitting(false)
    if (outcome.status === "failed") {
      setFailure(outcome.failure.message)
      return
    }
    setEditing(false)
    onChange()
  }

  async function deleteKey() {
    setSubmitting(true)
    setFailure(null)
    const outcome = await thinkingWorkspace.deleteCloudApiKey()
    setSubmitting(false)
    if (outcome.status === "failed") {
      setFailure(outcome.failure.message)
      return
    }
    setEditing(true)
    onChange()
  }

  if (!editing) {
    return (
      <div className="cloud-key">
        <p>A key is saved in the macOS keychain.</p>
        <div className="row">
          <button type="button" onClick={() => setEditing(true)} disabled={submitting}>
            Replace key
          </button>
          <button type="button" onClick={deleteKey} disabled={submitting}>
            Remove key
          </button>
        </div>
        {failure && <p role="alert">{failure}</p>}
      </div>
    )
  }

  return (
    <form className="cloud-key" onSubmit={saveKey}>
      <label htmlFor="cloud-api-key">Ollama Cloud key</label>
      <input
        id="cloud-api-key"
        type="password"
        autoComplete="off"
        spellCheck={false}
        placeholder="Paste your Ollama Cloud key"
        value={draft}
        onChange={(event) => setDraft(event.target.value)}
        disabled={submitting}
      />
      <p className="hint">The key is held in the macOS keychain and is never read back into Nodepad.</p>
      {failure && <p role="alert">{failure}</p>}
      <div className="row">
        <button type="submit" disabled={submitting || draft.trim() === ""}>
          Save key
        </button>
        {keyPresent && (
          <button type="button" onClick={() => { setEditing(false); setDraft(""); setFailure(null) }} disabled={submitting}>
            Cancel
          </button>
        )}
      </div>
    </form>
  )
}
