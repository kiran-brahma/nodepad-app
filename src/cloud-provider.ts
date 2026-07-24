import type { CloudProvider } from "./workspace-client"

export const CLOUD_PROVIDER_LABELS: Record<CloudProvider, string> = {
  ollama: "Ollama Cloud",
  openrouter: "OpenRouter",
  openai: "OpenAI",
  zai: "Z.ai",
}
