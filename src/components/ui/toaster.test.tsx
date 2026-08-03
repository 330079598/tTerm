// @vitest-environment jsdom

import { act, cleanup, fireEvent, render, screen } from "@testing-library/react"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { Toaster } from "@/components/ui/toaster"
import { toast } from "@/hooks/use-toast"

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
})
