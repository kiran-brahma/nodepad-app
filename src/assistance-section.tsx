import type { AssistancePolicy, ThinkingWorkspace } from "./workspace-client"
import type { DiscoveryState } from "./use-local-discovery"

const POLICY_LABELS: Record<AssistancePolicy, string> = {
  manual: "Manual",
  local_ai: "Local AI",
  cloud_ai: "Cloud AI",
}

/**
 * Controls for the Workspace's Assistance Policy and, when Local AI is
 * active, discovery/selection of an Ollama model. Nothing here sends Note
 * content; it only configures where future organization requests may go.
 */
export function AssistanceSection({
  activeWorkspace,
  state,
  query,
  filteredModels,
  selectedMissing,
  onPolicyChange,
  onQueryChange,
  onRefresh,
  onSelectModel,
}: {
  activeWorkspace: ThinkingWorkspace | undefined
  state: DiscoveryState
  query: string
  filteredModels: string[]
  selectedMissing: boolean
  onPolicyChange: (policy: AssistancePolicy) => void
  onQueryChange: (query: string) => void
  onRefresh: () => void
  onSelectModel: (modelId: string) => void
}) {
  if (!activeWorkspace) return null

  const policy = activeWorkspace.assistancePolicy

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
      </div>

      {policy === "local_ai" && (
        <div className="local-ai">
          <div className="row">
            <input
              aria-label="Search models"
              placeholder="Search models…"
              value={query}
              onChange={(event) => onQueryChange(event.target.value)}
            />
            <button onClick={onRefresh} disabled={state.kind === "loading"}>
              {state.kind === "loading" ? "Refreshing…" : "Refresh models"}
            </button>
          </div>

          {state.kind === "error" && (
            <p role="alert">{state.failure.message}</p>
          )}

          {state.kind === "ready" && selectedMissing && activeWorkspace.selectedModel && (
            <p role="alert">
              The selected model “{activeWorkspace.selectedModel}” is no longer available. Choose another.
            </p>
          )}

          {state.kind === "ready" && (
            <>
              {filteredModels.length === 0 ? (
                <p>No models match this search.</p>
              ) : (
                <ul aria-label="Available models">
                  {filteredModels.map((model) => (
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
    </section>
  )
}
