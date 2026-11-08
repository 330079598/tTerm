import type { SavedProfile } from "@/types/tab"
import exampleButtonInput from "@fixtures/example.button-input.default.json"
import buttonFixture from "@fixtures/ui.button.default.json"
import inputFixture from "@fixtures/ui.input.default.json"

/** What the mocked `invoke` answers for one screen. */
export interface ParityFixture {
  /** Fixture values keyed by the Tauri command name the screen calls. */
  commands: Record<string, unknown>
}

// `satisfies` is the type check the task card asks for: the JSON file keeps the
// data, and TypeScript verifies it against the same type the real command
// returns. A field renamed in src/types breaks this file at `pnpm typecheck`.
const exampleProfiles = exampleButtonInput.commands.list_profiles satisfies SavedProfile[]

/** Fixtures keyed by the screen id in parity/screens.json. */
export const fixtures: Record<string, ParityFixture> = {
  "example.button-input.default": {
    commands: {
      list_profiles: exampleProfiles,
    },
  },
  "ui.button.variants": { commands: buttonFixture.commands },
  "ui.button.sizes": { commands: buttonFixture.commands },
  "ui.input.default": { commands: inputFixture.commands },
}
