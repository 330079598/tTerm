// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { SerialSendHistoryMenu } from "@/components/TerminalTab/SerialSendHistoryMenu"
import type { SerialSend } from "@/lib/serialSendStore"

vi.mock("react-i18next", () => ({
  useTranslation: () => ({ t: (key: string) => key }),
}))

const history: SerialSend[] = [
  { id: "a", connectionKey: "port:COM3", mode: "text", text: "AT+RST", sentAt: 2 },
  { id: "b", connectionKey: "port:COM3", mode: "hex", text: "AA 55", sentAt: 1 },
]

function renderMenu(sends = history) {
  const handlers = { onPick: vi.fn(), onDelete: vi.fn(), onClear: vi.fn() }
  render(<SerialSendHistoryMenu history={sends} {...handlers} />)
  fireEvent.click(screen.getByLabelText("serialSend.history"))
  return handlers
}

describe("SerialSendHistoryMenu", () => {
  afterEach(() => {
    cleanup()
    vi.restoreAllMocks()
  })

  it("puts a picked send back and closes", () => {
    const { onPick } = renderMenu()
    fireEvent.click(screen.getByText("AA 55"))
    expect(onPick).toHaveBeenCalledWith(history[1])
    expect(screen.queryByRole("menu")).toBeNull()
  })

  it("forgets one send", () => {
    const { onDelete } = renderMenu()
    fireEvent.click(screen.getAllByLabelText("serialSend.historyDelete")[0])
    expect(onDelete).toHaveBeenCalledWith("a")
  })

  it("clears only once confirmed", () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false)
    const { onClear } = renderMenu()
    fireEvent.click(screen.getByText("serialSend.historyClear"))
    expect(onClear).not.toHaveBeenCalled()
    confirm.mockReturnValue(true)
    fireEvent.click(screen.getByText("serialSend.historyClear"))
    expect(onClear).toHaveBeenCalledOnce()
  })

  it("says when nothing was sent", () => {
    renderMenu([])
    expect(screen.getByText("serialSend.historyEmpty")).toBeTruthy()
    expect(screen.queryByText("serialSend.historyClear")).toBeNull()
  })
})
