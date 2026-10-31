// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen } from "@testing-library/react"
import type { TFunction } from "i18next"
import { afterEach, describe, expect, it, vi } from "vitest"
import type { Mock } from "vitest"

import type { useToast } from "@/hooks/use-toast"
import {
  describeInvalidField,
  useInvalidFieldReveal,
} from "@/components/ConnectionDialog/useInvalidFieldReveal"

const t = ((key: string, options?: Record<string, unknown>) =>
  options ? `${key} ${JSON.stringify(options)}` : key) as unknown as TFunction

type ToastFn = ReturnType<typeof useToast>["toast"]

function Harness({ toast, onSubmit }: { toast: Mock<ToastFn>; onSubmit: () => void }) {
  const handleInvalid = useInvalidFieldReveal({ toast, t })
  return (
    <form
      onSubmit={(event) => {
        event.preventDefault()
        onSubmit()
      }}
      onInvalidCapture={handleInvalid}
    >
      <label htmlFor="name">Name</label>
      <input id="name" />
      <label htmlFor="host">Host</label>
      <input id="host" required />
      <label htmlFor="port">Port</label>
      <input id="port" type="number" min={1} max={65535} defaultValue={0} />
      <button type="submit">Connect</button>
    </form>
  )
}

afterEach(cleanup)

describe("useInvalidFieldReveal", () => {
  it("focuses the first invalid field and names it in one toast", () => {
    const toast = vi.fn<ToastFn>()
    const onSubmit = vi.fn()
    render(<Harness toast={toast} onSubmit={onSubmit} />)

    fireEvent.click(screen.getByText("Connect"))

    expect(onSubmit).not.toHaveBeenCalled()
    expect(document.activeElement).toBe(screen.getByLabelText("Host"))
    expect(toast).toHaveBeenCalledTimes(1)
    expect(toast).toHaveBeenCalledWith({
      title: "connection.validation.title",
      description: 'connection.validation.required {"field":"Host"}',
      variant: "destructive",
    })
  })
})

describe("describeInvalidField", () => {
  it("gives the range of a number outside it", () => {
    render(
      <>
        <label htmlFor="port">Port</label>
        <input id="port" type="number" min={1} max={65535} defaultValue={70000} />
        <label htmlFor="baud">Baud</label>
        <input id="baud" type="number" min={1} defaultValue={0} />
      </>
    )

    expect(describeInvalidField(screen.getByLabelText("Port") as HTMLInputElement, t)).toBe(
      'connection.validation.between {"field":"Port","min":"1","max":"65535"}'
    )
    expect(describeInvalidField(screen.getByLabelText("Baud") as HTMLInputElement, t)).toBe(
      'connection.validation.atLeast {"field":"Baud","min":"1"}'
    )
  })
})
