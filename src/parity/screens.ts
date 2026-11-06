import type { ParityScreenProps } from "@/parity/types"
import { ExampleButtonInput } from "@/parity/screens/ExampleButtonInput"
import type { ComponentType } from "react"

/**
 * Every screen the reference capture can open, keyed by the id in
 * parity/screens.json. A screen id without an entry here renders the "unknown
 * screen" message, which the capture script treats as a failure.
 */
export const parityScreens: Record<string, ComponentType<ParityScreenProps>> = {
  "example.button-input.default": ExampleButtonInput,
}

/** The registered ids, for the manifest test. */
export const parityScreenIds = Object.keys(parityScreens).sort()
