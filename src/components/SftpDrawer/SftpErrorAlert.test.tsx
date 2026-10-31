// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { SftpErrorAlert } from "@/components/SftpDrawer/SftpErrorAlert"
import { isPermissionDenied } from "@/components/SftpDrawer/sftpDrawerUtils"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

afterEach(cleanup)

describe("isPermissionDenied", () => {
  it("knows SFTP's refusal, whatever its case, and nothing else", () => {
    expect(isPermissionDenied("Permission denied: Permission denied")).toBe(true)
    expect(isPermissionDenied("/srv/deploy.sh: permission denied")).toBe(true)
    expect(isPermissionDenied("No such file: No such file")).toBe(false)
    expect(isPermissionDenied(null)).toBe(false)
  })
})

describe("SftpErrorAlert", () => {
  it("shows nothing without an error", () => {
    render(<SftpErrorAlert error={null} onOpenAsRoot={vi.fn()} />)
    expect(screen.queryByRole("alert")).toBeNull()
  })

  it("offers to open as root only when asked to", () => {
    const openAsRoot = vi.fn()
    const { rerender } = render(<SftpErrorAlert error="No such file" />)
    expect(screen.queryByRole("button")).toBeNull()

    rerender(<SftpErrorAlert error="Permission denied" onOpenAsRoot={openAsRoot} />)
    fireEvent.click(screen.getByRole("button", { name: "sftp.sudo.openAsRoot" }))
    expect(openAsRoot).toHaveBeenCalledTimes(1)
  })
})
