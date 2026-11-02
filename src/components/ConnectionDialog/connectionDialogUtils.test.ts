import { describe, expect, it } from "vitest"

import {
  buildInitialForm,
  certificatePayload,
} from "@/components/ConnectionDialog/connectionDialogUtils"
import { defaultConfig, type AppConfig } from "@/lib/appConfig"
import type { SavedProfile } from "@/types/tab"

describe("certificatePayload", () => {
  it("sends a trimmed certificate only with key authentication", () => {
    expect(certificatePayload("key", " ~/.ssh/id-cert.pub ")).toBe("~/.ssh/id-cert.pub")
    expect(certificatePayload("key", "  ")).toBeUndefined()
    expect(certificatePayload("agent", "~/.ssh/id-cert.pub")).toBeUndefined()
    expect(certificatePayload("password", "~/.ssh/id-cert.pub")).toBeUndefined()
  })
})

describe("buildInitialForm", () => {
  const config: AppConfig = {
    ...defaultConfig,
    terminal_shell: "pwsh",
    local_shells: [{ id: "nu", name: "nu", path: "/bin/nu", args: "" }],
  }
  const local = (local_terminal: SavedProfile["local_terminal"]): SavedProfile => ({
    id: "p",
    name: "web",
    connection_type: "terminal",
    remember_password: false,
    keepalive_interval_secs: 15,
    keepalive_count_max: 3,
    local_terminal,
  })

  it("shows a saved local terminal's own shell and directory", () => {
    const form = buildInitialForm(
      local({ shell: "custom", customPath: "/bin/nu", shellId: "nu", startDirectory: "~/web" }),
      config
    )
    expect(form).toMatchObject({
      type: "terminal",
      terminalShell: "custom",
      terminalShellKey: "custom:nu",
      startDirectory: "~/web",
    })
  })

  it("shows the default shell for a new local terminal", () => {
    expect(buildInitialForm(null, config)).toMatchObject({
      terminalShell: "pwsh",
      terminalShellKey: "pwsh",
      startDirectory: "",
    })
  })
})
