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
  },
})
