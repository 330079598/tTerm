// @vitest-environment jsdom

import { useState } from "react"
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react"
import { afterEach, describe, expect, it } from "vitest"

import { useSftpConflictDialog } from "@/components/SftpDrawer/SftpConflictDialog"
import type { ConflictReport } from "@/components/SftpDrawer/types"

afterEach(cleanup)

const report: ConflictReport = {
  conflicts: [
    {
      sourcePath: "/local/nginx.conf",
      sourceSize: 2048,
      sourceMtime: 1_700_000_100,
      targetIsDir: false,
      targetPath: "/etc/nginx/nginx.conf",
      targetSize: 1024,
      targetMtime: 1_700_000_000,
    },
  ],
  fileCount: 60,
  total: 51,
}

function Harness() {
  const { conflictDialog, promptConflictPolicy } = useSftpConflictDialog()
  const [result, setResult] = useState("pending")

  return (
    <>
      <button
        type="button"
        onClick={async () => setResult(String(await promptConflictPolicy(report, "upload")))}
      >
        Upload
      </button>
      <output>{result}</output>
      {conflictDialog}
    </>
  )
}

describe("useSftpConflictDialog", () => {
  it("lists the conflicts and resolves with the chosen policy", async () => {
    render(<Harness />)
    fireEvent.click(screen.getByText("Upload"))

    expect(await screen.findByText("nginx.conf")).toBeTruthy()
    // 51 conflicts, one listed: the rest are summarised.
    expect(screen.getByText("and 50 more")).toBeTruthy()

    fireEvent.click(screen.getByLabelText(/Keep both/))
    fireEvent.click(screen.getByText("Continue"))

    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("rename"))
  })

  it("resolves with null when the transfer is cancelled", async () => {
    render(<Harness />)
    fireEvent.click(screen.getByText("Upload"))
    fireEvent.click(await screen.findByText("Cancel transfer"))

    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("null"))
  })

  it("preselects the previous choice for the next prompt", async () => {
    render(<Harness />)
    fireEvent.click(screen.getByText("Upload"))
    fireEvent.click(await screen.findByLabelText(/Skip all/))
    fireEvent.click(screen.getByText("Continue"))
    await waitFor(() => expect(screen.getByRole("status").textContent).toBe("skip"))

    fireEvent.click(screen.getByText("Upload"))
    const skip = (await screen.findByLabelText(/Skip all/)) as HTMLInputElement
    expect(skip.checked).toBe(true)
  })
})
