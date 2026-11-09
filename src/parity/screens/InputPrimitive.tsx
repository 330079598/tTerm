import { Input } from "@/components/ui/input"
import type { ParityScreenProps } from "@/parity/types"
import { useEffect } from "react"
export function InputPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main
      data-parity="ui.input.screen"
      className="bg-background text-foreground h-full w-full p-8 text-sm"
    >
      <div className="flex max-w-xl flex-col gap-4">
        <Input data-parity="ui.input.empty" aria-label="Empty" placeholder="Placeholder" />
        <Input
          data-parity="ui.input.long"
          aria-label="Long"
          value="A long input value used to verify clipping and horizontal layout"
          readOnly
        />
        <Input data-parity="ui.input.focus" aria-label="Focus" placeholder="Focus me" autoFocus />
        <Input
          data-parity="ui.input.disabled"
          aria-label="Disabled"
          value="Disabled"
          disabled
          readOnly
        />
      </div>
    </main>
  )
}
