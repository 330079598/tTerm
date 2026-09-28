// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import {
  BandwidthLimitInput,
  formatLimit,
  parseLimit,
  preferredUnit,
} from "@/components/SettingsDialog/BandwidthLimitInput"

afterEach(cleanup)

function renderInput(value: number) {
  const onChange = vi.fn()
  render(<BandwidthLimitInput aria-label="Upload speed limit" value={value} onChange={onChange} />)
  const input = screen.getByLabelText("Upload speed limit") as HTMLInputElement
  const unit = screen.getByLabelText("Unit") as HTMLSelectElement
  return { input, onChange, unit }
}

describe("bandwidth limit helpers", () => {
  it("reads whole MB/s limits in MB/s and everything else in KB/s", () => {
    expect(preferredUnit(0)).toBe("KB")
    expect(preferredUnit(2048)).toBe("MB")
    expect(preferredUnit(1500)).toBe("KB")
    expect(formatLimit(2048, "MB")).toBe("2")
    expect(formatLimit(100, "MB")).toBe("0.1")
  })

  it("parses to KiB/s, clamps, and rejects non-numbers", () => {
    expect(parseLimit("1.5", "MB")).toBe(1536)
    expect(parseLimit("300", "KB")).toBe(300)
    expect(parseLimit("-5", "KB")).toBe(0)
    expect(parseLimit("", "KB")).toBeNull()
    expect(parseLimit("abc", "KB")).toBeNull()
    expect(parseLimit("99999999", "MB")).toBe(10 * 1024 * 1024)
  })
})

describe("BandwidthLimitInput", () => {
  it("commits on blur, not per keystroke", () => {
    const { input, onChange } = renderInput(0)

    fireEvent.change(input, { target: { value: "1" } })
    fireEvent.change(input, { target: { value: "1024" } })
    expect(onChange).not.toHaveBeenCalled()

    fireEvent.blur(input)
    expect(onChange).toHaveBeenCalledTimes(1)
    expect(onChange).toHaveBeenCalledWith(1024)
  })

  it("commits on Enter in the chosen unit", () => {
    const { input, onChange, unit } = renderInput(0)

    fireEvent.change(unit, { target: { value: "MB" } })
    fireEvent.change(input, { target: { value: "5" } })
    fireEvent.keyDown(input, { key: "Enter" })

    expect(onChange).toHaveBeenCalledWith(5 * 1024)
  })

  it("keeps the rate when switching units and does not re-save a rounded display", () => {
    const { input, onChange, unit } = renderInput(100)
    expect(input.value).toBe("100")

    fireEvent.change(unit, { target: { value: "MB" } })
    expect(input.value).toBe("0.1")
    fireEvent.blur(input)
    expect(onChange).not.toHaveBeenCalled()
  })

  it("restores the stored value when the input is cleared", () => {
    const { input, onChange } = renderInput(512)

    fireEvent.change(input, { target: { value: "" } })
    fireEvent.blur(input)

    expect(onChange).not.toHaveBeenCalled()
    expect(input.value).toBe("512")
  })
})
