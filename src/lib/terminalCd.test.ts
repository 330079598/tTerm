import { describe, expect, it } from "vitest"

import { terminalCdCommand } from "@/lib/terminalCd"

describe("terminalCdCommand", () => {
  it("escapes what the shell would read as syntax and presses Enter", () => {
    expect(terminalCdCommand("/var/log")).toBe("cd /var/log\r")
    expect(terminalCdCommand("/srv/my app/$HOME")).toBe("cd /srv/my\\ app/\\$HOME\r")
    expect(terminalCdCommand("/tmp/it's")).toBe("cd /tmp/it\\'s\r")
  })
})
