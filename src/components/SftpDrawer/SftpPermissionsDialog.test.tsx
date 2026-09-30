// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { SftpPermissionsDialog } from "@/components/SftpDrawer/SftpPermissionsDialog"
import type { SftpDirectoryEntry } from "@/components/SftpDrawer/types"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const script: SftpDirectoryEntry = {
  name: "deploy.sh",
  path: "/srv/deploy.sh",
  isDir: false,
  isSymlink: false,
  mode: 0o644,
}

afterEach(cleanup)

describe("SftpPermissionsDialog", () => {
  it("keeps the checkboxes and the octal field in step and applies the mode", async () => {
    const onApply = vi.fn(async () => undefined)
    const onClose = vi.fn()
    render(<SftpPermissionsDialog entries={[script]} onApply={onApply} onClose={onClose} />)

    const octal = screen.getByLabelText("sftp.permissions.octal")
    expect(octal).toHaveProperty("value", "644")
    const ownerExecute = screen.getByRole("checkbox", {
      name: "sftp.permissions.owner sftp.permissions.execute",
    })
    expect(ownerExecute.getAttribute("aria-checked")).toBe("false")

    fireEvent.click(ownerExecute)
    expect(octal).toHaveProperty("value", "744")

    fireEvent.change(octal, { target: { value: "2755x" } })
    expect(octal).toHaveProperty("value", "2755")
    expect(
      screen
        .getByRole("checkbox", { name: "sftp.permissions.others sftp.permissions.write" })
        .getAttribute("aria-checked")
    ).toBe("false")
    expect(screen.getByText("sftp.permissions.specialHint")).toBeTruthy()

    fireEvent.submit(octal.closest("form")!)
    await waitFor(() => expect(onApply).toHaveBeenCalledWith([script], 0o2755))
    await waitFor(() => expect(onClose).toHaveBeenCalled())
  })

  it("shows the failure and stays open", async () => {
    const onApply = vi.fn(async () => {
      throw new Error("/srv/deploy.sh: Permission denied")
    })
    const onClose = vi.fn()
    render(<SftpPermissionsDialog entries={[script]} onApply={onApply} onClose={onClose} />)

    fireEvent.submit(screen.getByLabelText("sftp.permissions.octal").closest("form")!)
    expect((await screen.findByRole("alert")).textContent).toContain("Permission denied")
    expect(onClose).not.toHaveBeenCalled()
  })

  it("refuses an incomplete mode and renders nothing without entries", () => {
    const { rerender } = render(
      <SftpPermissionsDialog entries={[script]} onApply={vi.fn()} onClose={vi.fn()} />
    )
    fireEvent.change(screen.getByLabelText("sftp.permissions.octal"), { target: { value: "7" } })
    expect(screen.getByRole("button", { name: "sftp.permissions.apply" })).toHaveProperty(
      "disabled",
      true
    )

    rerender(<SftpPermissionsDialog entries={[]} onApply={vi.fn()} onClose={vi.fn()} />)
    expect(screen.queryByLabelText("sftp.permissions.octal")).toBeNull()
  })
})
