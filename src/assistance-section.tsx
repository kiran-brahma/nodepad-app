import type {
  AssistancePolicy,
  CloudProvider,
  DiscoveryState,
  ThinkingWorkspace,
} from "./workspace-client"
import { CloudConsentDialog } from "./cloud-consent-dialog"
import { CloudKeySection } from "./cloud-key-section"
import { CLOUD_PROVIDER_LABELS } from "./cloud-provider"

const POLICY_LABELS: Record<AssistancePolicy, string> = {
  manual: "Manual",
  local_ai: "Local AI",
  cloud_ai: "Cloud AI",
}

/**
 * Controls for the Workspace's Assistance Policy and, when AI is active,
 * discovery/selection of an Ollama model. The Cloud AI branch requires a
 * per-Workspace consent disclosure and a key saved in the macOS keychain
 * before any cloud call is made. Nothing here sends Note content; it only
 * configures where future organization requests may go.
 */
export function AssistanceSection({
  activeWorkspace,
  localState,
  localQuery,
  localFilteredModels,
  cloudState,
  cloudQuery,
  cloudFilteredModels,
  cloudKeyPresent,
  selectedMissing,
  onPolicyChange,
  onCloudProviderChange,
  onLocalQueryChange,
  onLocalRefresh,
  onCloudQueryChange,
  onCloudRefresh,
  onCloudKeyChange,
  onRequestCloudConsent,
  onRevokeCloudConsent,
  onSelectModel,
}: {
  activeWorkspace: ThinkingWorkspace | undefined
  localState: DiscoveryState
  localQuery: string
  localFilteredModels: string[]
  cloudState: DiscoveryState
  cloudQuery: string
  cloudFilteredModels: string[]
  cloudKeyPresent: boolean
  selectedMissing: boolean
  onPolicyChange: (policy: AssistancePolicy) => void
  onCloudProviderChange: (provider: CloudProvider) => void
  onLocalQueryChange: (query: string) => void
  onLocalRefresh: () => void
  onCloudQueryChange: (query: string) => void
  onCloudRefresh: () => void
  onCloudKeyChange: () => void
  onRequestCloudConsent: () => void
  onRevokeCloudConsent: () => void
  onSelectModel: (modelId: string) => void
}) {
  if (!activeWorkspace) return null

  const policy = activeWorkspace.assistancePolicy
  const consented = activeWorkspace.cloudConsentAt !== null
  const cloudProvider = activeWorkspace.cloudProvider ?? "ollama"

  return (
    <section aria-label="AI Assistance">
      <h2>AI Assistance</h2>
      <p>{activeWorkspace.name} is using {POLICY_LABELS[policy]} assistance.</p>

      <div className="row" role="group" aria-label="Assistance Policy">
        <button
          aria-pressed={policy === "manual"}
          className={policy === "manual" ? "active" : ""}
          onClick={() => onPolicyChange("manual")}
        >
          Manual
        </button>
        <button
          aria-pressed={policy === "local_ai"}
          className={policy === "local_ai" ? "active" : ""}
          onClick={() => onPolicyChange("local_ai")}
        >
          Local AI
        </button>
        <button
          aria-pressed={policy === "cloud_ai"}
          className={policy === "cloud_ai" ? "active" : ""}
          onClick={() => onPolicyChange("cloud_ai")}
        >
          Cloud AI
        </button>
      </div>

      {policy === "local_ai" && (
        <div className="local-ai">
          <div className="row">
            <input
              aria-label="Search models"
              placeholder="Search models…"
              value={localQuery}
              onChange={(event) => onLocalQueryChange(event.target.value)}
            />
            <button onClick={onLocalRefresh} disabled={localState.kind === "loading"}>
              {localState.kind === "loading" ? "Refreshing…" : "Refresh models"}
            </button>
          </div>

          {localState.kind === "error" && (
            <p role="alert">{localState.failure.message}</p>
          )}

          {localState.kind === "ready" && selectedMissing && activeWorkspace.selectedModel && (
            <p role="alert">
              The selected model “{activeWorkspace.selectedModel}” is no longer available. Choose another.
            </p>
          )}

          {localState.kind === "ready" && (
            <>
              {localFilteredModels.length === 0 ? (
                <p>No models match this search.</p>
              ) : (
                <ul aria-label="Available models">
                  {localFilteredModels.map((model) => (
                    <li key={model} className="row">
                      <span>{model}</span>
                      {activeWorkspace.selectedModel === model ? (
                        <span>Selected</span>
                      ) : (
                        <button onClick={() => onSelectModel(model)}>Select</button>
                      )}
                    </li>
                  ))}
                </ul>
              )}
            </>
          )}
        </div>
      )}

      {policy === "cloud_ai" && (
        <div className="cloud-ai">
          <label>
            Cloud provider
            <select
              aria-label="Cloud provider"
              value={cloudProvider}
              onChange={(event) => onCloudProviderChange(event.target.value as CloudProvider)}
            >
              <option value="ollama">Ollama Cloud</option>
              <option value="openrouter">OpenRouter</option>
              <option value="openai">OpenAI</option>
              <option value="zai">Z.ai</option>
            </select>
          </label>
          {!consented && (
            <div>
              <p>This Workspace has not consented to Cloud AI yet.</p>
              <button onClick={onRequestCloudConsent}>Read the disclosure</button>
            </div>
          )}

          {consented && (
            <>
              <CloudKeySection
                keyPresent={cloudKeyPresent}
                provider={cloudProvider}
                onChange={onCloudKeyChange}
              />

              {!cloudKeyPresent && (
                <p>Add your {CLOUD_PROVIDER_LABELS[cloudProvider]} key to discover cloud-hosted models.</p>
              )}

              {cloudKeyPresent && (
                <>
                  <div className="row">
                    <input
                      aria-label="Search cloud models"
                      placeholder="Search models…"
                      value={cloudQuery}
                      onChange={(event) => onCloudQueryChange(event.target.value)}
                    />
                    <button onClick={onCloudRefresh} disabled={cloudState.kind === "loading"}>
                      {cloudState.kind === "loading" ? "Refreshing…" : "Refresh models"}
                    </button>
                  </div>

                  {cloudState.kind === "error" && (
                    <p role="alert">{cloudState.failure.message}</p>
                  )}

                  {cloudState.kind === "ready" && selectedMissing && activeWorkspace.selectedModel && (
                    <p role="alert">
                      The selected model “{activeWorkspace.selectedModel}” is no longer available. Choose another.
                    </p>
                  )}

                  {cloudState.kind === "ready" && (
                    <>
                      {cloudFilteredModels.length === 0 ? (
                        <p>No models match this search.</p>
                      ) : (
                        <ul aria-label="Available cloud models">
                          {cloudFilteredModels.map((model) => (
                            <li key={model} className="row">
                              <span>{model}</span>
                              {activeWorkspace.selectedModel === model ? (
                                <span>Selected</span>
                              ) : (
                                <button onClick={() => onSelectModel(model)}>Select</button>
                              )}
                            </li>
                          ))}
                        </ul>
                      )}
                    </>
                  )}
                </>
              )}

              <div>
                <button onClick={onRevokeCloudConsent}>Revoke Cloud consent for this Workspace</button>
              </div>
            </>
          )}
        </div>
      )}
    </section>
  )
}

/** A small re-export so App can open the dialog from one entry point. */
export { CloudConsentDialog }
