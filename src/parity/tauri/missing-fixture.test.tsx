// @vitest-environment jsdom
import { render, screen, waitFor } from "@testing-library/react"
import { useEffect, useState } from "react"
import { beforeEach, describe, expect, it } from "vitest"
import { activateFixture, invoke, resetParityCore } from "@/parity/tauri/core"

/** A component that swallows the rejection, the case capture must still catch. */
function CatchesMissingInvoke() {
  const [state, setState] = useState("loading")

  useEffect(() => {
    invoke("missing_from_component")
      .then(() => setState("loaded"))
      .catch(() => setState("failed"))
  }, [])

  return <div>{state}</div>
}

beforeEach(() => {
  resetParityCore()
  activateFixture({ commands: {} })
})

describe("missing fixture tracking", () => {
  it("records a command even when the component catches the rejection", async () => {
    render(<CatchesMissingInvoke />)
    await waitFor(() => expect(screen.getByText("failed")).toBeTruthy())
    expect(window.__PARITY_MISSING__).toEqual(["missing_from_component"])
  })
})
