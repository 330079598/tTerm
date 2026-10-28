import { describe, expect, it } from "vitest"

import { FISH_SHELL_INTEGRATION, POSIX_SHELL_INTEGRATION } from "@/lib/remoteShellIntegration"

describe("remote shell integration scripts", () => {
  it("report the directory with `%` escaped, as the shell expands it", () => {
    // Both the zsh and the bash prompt function send it.
    expect(POSIX_SHELL_INTEGRATION.split("file://%s").length - 1).toBe(2)
    expect(POSIX_SHELL_INTEGRATION.split('"${PWD//\\%/%25}"').length - 1).toBe(2)
    expect(FISH_SHELL_INTEGRATION).toContain("(string replace --all '%' '%25' -- $PWD)")
  })

  it("name the host, so a shell on a host reached with ssh is told apart", () => {
    expect(POSIX_SHELL_INTEGRATION.split("file://%s%s").length - 1).toBe(2)
    expect(POSIX_SHELL_INTEGRATION).toContain('"$?" "$HOST" "${PWD')
    expect(POSIX_SHELL_INTEGRATION).toContain('"$s" "$HOSTNAME" "${PWD')
    expect(FISH_SHELL_INTEGRATION).toContain("file://%s%s")
    expect(FISH_SHELL_INTEGRATION).toContain("' $hostname (string replace")
  })
})
