// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { Toaster } from "@/components/ui/toaster"
import { setToastLimit, toast } from "@/hooks/use-toast"

describe("Toaster", () => {
  beforeEach(() => {
    render(<Toaster />)
  })

  afterEach(cleanup)

  it("notifies the toast owner when the user closes it", () => {
    const onOpenChange = vi.fn()
    act(() => {
      toast({ title: "Downloading update", duration: Number.POSITIVE_INFINITY, onOpenChange })
    })

    fireEvent.click(screen.getByRole("button"))

    expect(onOpenChange).toHaveBeenCalledWith(false)
  })

  it("stacks up to its limit, closing the oldest past it", () => {
    act(() => {
      setToastLimit(2)
      for (const title of ["First", "Second", "Third"]) {
        toast({ title, duration: Number.POSITIVE_INFINITY })
      }
    })

    expect(screen.queryByText("First")).toBeNull()
    expect(screen.getByText("Second")).not.toBeNull()
    expect(screen.getByText("Third")).not.toBeNull()

    act(() => {
      setToastLimit(1)
    })
    expect(screen.queryByText("Second")).toBeNull()
    expect(screen.getByText("Third")).not.toBeNull()

    act(() => {
      setToastLimit(3)
    })
  })

  describe("closing on its own", () => {
    beforeEach(() => {
      vi.useFakeTimers()
    })

    afterEach(() => {
      act(() => {
        vi.runOnlyPendingTimers()
      })
      vi.useRealTimers()
    })

    it("closes after its duration", () => {
      act(() => {
        toast({ title: "Command finished", duration: 3000 })
      })

      act(() => {
        vi.advanceTimersByTime(3000)
      })

      expect(screen.queryByText("Command finished")).toBeNull()
    })

    it("waits while the pointer is on it", () => {
      act(() => {
        toast({ title: "Command finished", duration: 3000 })
      })
      act(() => {
        vi.advanceTimersByTime(1000)
      })

      fireEvent.pointerMove(screen.getByText("Command finished"))
      act(() => {
        vi.advanceTimersByTime(10_000)
      })
      expect(screen.getByText("Command finished")).not.toBeNull()

      fireEvent.pointerLeave(screen.getByRole("region"))
      act(() => {
        vi.advanceTimersByTime(1999)
      })
      expect(screen.getByText("Command finished")).not.toBeNull()
      act(() => {
        vi.advanceTimersByTime(1)
      })
      expect(screen.queryByText("Command finished")).toBeNull()
    })
  })
})
