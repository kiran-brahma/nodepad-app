import { defineConfig } from "vitest/config"
import react from "@vitejs/plugin-react"

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
  },
  test: {
    // UI tests drive the real components through the DOM rather than inspecting
    // React state, so they need a DOM.
    environment: "jsdom",
    // Keep vitest's defaults and add the agent worktrees under `.claude/`, which
    // are full checkouts of this repo with their own stale copies of the tests.
    exclude: [
      "**/node_modules/**",
      "**/dist/**",
      "**/cypress/**",
      "**/.{idea,git,cache,output,temp}/**",
      "**/.claude/**",
      "**/.pi-subagents/**",
      "**/{karma,rollup,webpack,vite,vitest,jest,ava,babel,nyc,cypress,tsup,build}.config.*",
    ],
  },
})
