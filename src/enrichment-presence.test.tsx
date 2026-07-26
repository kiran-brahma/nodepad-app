import { cleanup, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it } from "vitest"
import { EnrichmentPresence } from "./enrichment-presence"

afterEach(cleanup)

describe("background enrichment presence", () => {
  it("names a running request as working and every other enabled state as quiet", () => {
    const { rerender } = render(
      <EnrichmentPresence
        enabled
        working
      />,
    )
    expect(screen.getByRole("status").textContent).toBe("AI · working")

    rerender(<EnrichmentPresence enabled working={false} />)
    expect(screen.getByRole("status").textContent).toBe("AI · quiet")
  })

  it("renders no AI presence for a Manual Thinking Workspace", () => {
    render(<EnrichmentPresence enabled={false} working />)
    expect(screen.queryByRole("status")).toBeNull()
  })
})
