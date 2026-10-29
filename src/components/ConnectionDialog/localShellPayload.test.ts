import { describe, expect, it } from "vitest"

import {
  buildLocalShellSubmit,
  NEW_SHELL_KEY,
} from "@/components/ConnectionDialog/localShellPayload"
import { defaultForm, type ConnectionForm } from "@/components/ConnectionDialog/types"
import type { ShellConfig } from "@/lib/localShells"

const config: ShellConfig = {
  terminal_shell: "auto",
  terminal_shell_custom_path: "",
  terminal_shell_custom_args: "",
  terminal_shell_id: "",
  local_shells: [{ id: "nu", name: "nu", path: "nu", args: "" }],
  hidden_shells: [],
}

const form = (overrides: Partial<ConnectionForm>): ConnectionForm => ({
  ...defaultForm,
  type: "terminal",
  ...overrides,
})

describe("buildLocalShellSubmit", () => {
  it("opens a shell from the list without changing the settings", () => {
    const { connection, updates } = buildLocalShellSubmit(
      form({
        terminalShellKey: "custom:nu",
        terminalShell: "custom",
        terminalShellCustomPath: "nu",
        terminalShellId: "nu",
      }),
      config
    )
    expect(connection).toMatchObject({ terminalShellCustomPath: "nu", terminalShellId: "nu" })
    expect(updates).toBeNull()
  })

  it("opens a shell typed in once, adding it only when asked", () => {
    const typed = form({
      terminalShellKey: NEW_SHELL_KEY,
      terminalShell: "custom",
      terminalShellCustomPath: " /opt/bin/xonsh ",
      terminalShellCustomArgs: "-i",
    })
    const once = buildLocalShellSubmit(typed, config)
    expect(once.updates).toBeNull()
    expect(once.connection).toMatchObject({
      terminalShellCustomPath: "/opt/bin/xonsh",
      terminalShellCustomArgs: "-i",
      terminalShellId: undefined,
    })

    const saved = buildLocalShellSubmit({ ...typed, saveShell: true }, config)
    const added = saved.updates?.local_shells?.[1]
    expect(added).toMatchObject({ name: "xonsh", path: "/opt/bin/xonsh", args: "-i" })
    expect(saved.connection.terminalShellId).toBe(added?.id)
    expect(saved.updates?.terminal_shell).toBeUndefined()
  })

  it("adds a shell typed in when it is made the default, and points the default at it", () => {
    const { connection, updates } = buildLocalShellSubmit(
      form({
        terminalShellKey: NEW_SHELL_KEY,
        terminalShell: "custom",
        terminalShellCustomPath: "fish",
        makeDefaultShell: true,
      }),
      config
    )
    const id = connection.terminalShellId
    expect(id).toBeTruthy()
    expect(updates).toMatchObject({
      terminal_shell: "custom",
      terminal_shell_custom_path: "fish",
      terminal_shell_id: id,
    })
    expect(updates?.local_shells?.map((shell) => shell.id)).toEqual(["nu", id])
  })

  it("makes a detected shell the default", () => {
    const { updates } = buildLocalShellSubmit(
      form({ terminalShellKey: "pwsh", terminalShell: "pwsh", makeDefaultShell: true }),
      config
    )
    expect(updates).toEqual({
      terminal_shell: "pwsh",
      terminal_shell_custom_path: "",
      terminal_shell_custom_args: "",
      terminal_shell_id: "",
    })
  })
})
