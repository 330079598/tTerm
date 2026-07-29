// @vitest-environment jsdom

import { useState } from "react"
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it } from "vitest"

import { useConfirmDialog } from "@/components/ui/app-dialog"

afterEach(cleanup)

function ConfirmDialogHarness() {
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [result, setResult] = useState("pending")

  return (
    <>
      <button
        type="button"
        onClick={async () => {
          const confirmed = await confirm({
            title: "Broadcast multiline paste?",
            confirmText: "Send anyway",
            defaultAction: "confirm",
          })
          setResult(confirmed ? "confirmed" : "cancelled")
        }}
      >
        Open confirmation
      </button>
      <output>{result}</output>
      <ConfirmDialog />
    </>
  )
}

describe("useConfirmDialog", () => {
  it("confirms the default action with Enter and restores the previous focus", async () => {
    render(<ConfirmDialogHarness />)
    const trigger = screen.getByRole("button", { name: "Open confirmation" })
    trigger.focus()
    fireEvent.click(trigger)

    const confirmButton = await screen.findByRole("button", { name: "Send anyway" })
    expect(document.activeElement).toBe(confirmButton)

    fireEvent.keyDown(confirmButton, { key: "Enter" })

    await waitFor(() => expect(screen.getByText("confirmed")).toBeTruthy())
    expect(document.activeElement).toBe(trigger)
  })
})
