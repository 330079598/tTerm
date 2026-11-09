import type { ParityScreenProps } from "@/parity/types"
import { ExampleButtonInput } from "@/parity/screens/ExampleButtonInput"
import { ButtonVariants, ButtonSizes } from "@/parity/screens/ButtonPrimitive"
import { InputPrimitive } from "@/parity/screens/InputPrimitive"
import {
  PasswordInputPrimitive,
  SelectPrimitive,
  SwitchPrimitive,
} from "@/parity/screens/P024Primitives"
import type { ComponentType } from "react"

/**
 * Every screen the reference capture can open, keyed by the id in
 * parity/screens.json. A screen id without an entry here renders the "unknown
 * screen" message, which the capture script treats as a failure.
 */
export const parityScreens: Record<string, ComponentType<ParityScreenProps>> = {
  "example.button-input.default": ExampleButtonInput,
  "ui.button.variants": ButtonVariants,
  "ui.button.sizes": ButtonSizes,
  "ui.input.default": InputPrimitive,
  "ui.password.default": PasswordInputPrimitive,
  "ui.select.default": SelectPrimitive,
  "ui.switch.default": SwitchPrimitive,
}

/** The registered ids, for the manifest test. */
export const parityScreenIds = Object.keys(parityScreens).sort()
