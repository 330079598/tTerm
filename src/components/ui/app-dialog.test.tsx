// @vitest-environment jsdom

import { useState } from "react"
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it } from "vitest"

import { useConfirmDialog } from "@/components/ui/app-dialog"
import { Dialog, DialogContent, DialogTitle } from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"

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

function InputFocusDialogHarness({ explicitInitialFocus = false }: { explicitInitialFocus?: boolean }) {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open input dialog
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        {open && (
          <DialogContent>
            <DialogTitle>Input dialog</DialogTitle>
            <Input aria-label="Dialog input" />
            {explicitInitialFocus && (
              <button type="button" data-dialog-initial-focus>
                Explicit focus
              </button>
            )}
          </DialogContent>
        )}
      </Dialog>
    </>
  )
}

function DisabledInputFocusDialogHarness() {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open input dialog
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        {open && (
          <DialogContent>
            <DialogTitle>Input dialog</DialogTitle>
            <Input aria-label="Disabled input" disabled />
            <Input aria-label="Enabled input" />
          </DialogContent>
        )}
      </Dialog>
    </>
  )
}

function AutoFocusDialogHarness() {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open auto focus dialog
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        {open && (
          <DialogContent>
            <DialogTitle>Auto focus dialog</DialogTitle>
            <Input aria-label="First input" />
            <Input aria-label="Auto focused input" autoFocus />
          </DialogContent>
        )}
      </Dialog>
    </>
  )
}

function EmptyFocusDialogHarness() {
  const [open, setOpen] = useState(false)

  return (
    <>
      <button type="button" onClick={() => setOpen(true)}>
        Open empty dialog
      </button>
      <Dialog open={open} onOpenChange={setOpen}>
        {open && (
          <DialogContent>
            <DialogTitle>Empty dialog</DialogTitle>
          </DialogContent>
        )}
      </Dialog>
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

  it("focuses the first enabled input when a dialog opens", async () => {
    render(<InputFocusDialogHarness />)
    const opener = screen.getByRole("button", { name: "Open input dialog" })
    opener.focus()
    fireEvent.click(opener)

    const input = await screen.findByRole("textbox")
    expect(document.activeElement).toBe(input)
  })

  it("prefers an explicit initial-focus element over an input", async () => {
    render(<InputFocusDialogHarness explicitInitialFocus />)
    fireEvent.click(screen.getByRole("button", { name: "Open input dialog" }))

    const markedButton = await screen.findByRole("button", { name: "Explicit focus" })
    expect(document.activeElement).toBe(markedButton)
  })

  it("skips a disabled input when choosing initial focus", async () => {
    render(<DisabledInputFocusDialogHarness />)
    fireEvent.click(screen.getByRole("button", { name: "Open input dialog" }))

    const enabledInput = await screen.findByRole("textbox", { name: "Enabled input" })
    expect(document.activeElement).toBe(enabledInput)
  })

  it("preserves an input's explicit autoFocus when another input comes first", async () => {
    render(<AutoFocusDialogHarness />)
    fireEvent.click(screen.getByRole("button", { name: "Open auto focus dialog" }))

    const autoFocusedInput = await screen.findByRole("textbox", { name: "Auto focused input" })
    expect(document.activeElement).toBe(autoFocusedInput)
  })

  it("focuses the dialog container when it has no form control", async () => {
    render(<EmptyFocusDialogHarness />)
    fireEvent.click(screen.getByRole("button", { name: "Open empty dialog" }))

    const dialog = await screen.findByRole("dialog")
    expect(document.activeElement).toBe(dialog)
  })
})
