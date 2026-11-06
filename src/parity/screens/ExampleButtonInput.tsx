import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import type { ParityScreenProps } from "@/parity/types"
import type { SavedProfile } from "@/types/tab"
import { invoke } from "@tauri-apps/api/core"
import { useEffect, useState } from "react"

/**
 * The example screen W0-04 compares: one button and one input.
 *
 * It exists to exercise the whole pipeline, not to mirror a product screen. It
 * still uses the real `Button` and `Input`, and its initial value comes from the
 * `list_profiles` fixture through the mocked `invoke`, so the reference run
 * proves the fixture mock is what the component actually calls.
 *
 * The inline line height and the explicit radius override the component
 * defaults on purpose: the tool compares line heights exactly, and GPUI rounds
 * a line height to whole pixels while CSS does not. Sixteen pixels is exact on
 * both sides. Everything else is left to the components.
 */
export function ExampleButtonInput({ onReady }: ParityScreenProps) {
  const [name, setName] = useState("")

  useEffect(() => {
    let cancelled = false
    invoke<SavedProfile[]>("list_profiles")
      .then((profiles) => {
        if (!cancelled) setName(profiles[0]?.name ?? "")
      })
      .finally(() => {
        if (!cancelled) onReady()
      })
    return () => {
      cancelled = true
    }
  }, [onReady])

  return (
    <div
      data-parity="example.screen"
      className="bg-background text-foreground flex h-full w-full flex-col gap-4 p-8"
    >
      <span className="text-muted-foreground text-sm leading-4">Profile name</span>
      <div className="flex items-center gap-2">
        <Input
          data-parity="example.input"
          aria-label="Profile name"
          readOnly
          value={name}
          className="border-input h-9 w-72 rounded-[4px] px-3 py-1 text-sm leading-4 shadow-none dark:bg-transparent"
          style={{ lineHeight: "16px" }}
        />
        <Button
          data-parity="example.button"
          data-parity-sizing="text"
          type="button"
          className="h-9 rounded-[4px] px-4 py-2 text-sm leading-4 shadow-none"
          style={{ lineHeight: "16px" }}
        >
          Save
        </Button>
      </div>
    </div>
  )
}
