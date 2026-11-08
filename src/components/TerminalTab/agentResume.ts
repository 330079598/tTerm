import { invoke } from "@tauri-apps/api/core"
import type { Terminal } from "@xterm/xterm"

import type { CommandMarks } from "@/components/TerminalTab/commandMarks"
import type { AgentResumeInfo } from "@/types/tab"

const RESUME_TIMEOUT_MS = 15_000

interface AgentResumeOptions {
  term: Terminal
  commandMarks: CommandMarks
  resume: AgentResumeInfo | undefined
  cwd: string | undefined
  shell: string | undefined
  tabId: string
  sessionNonce: number
}

/** Sends only the command returned by the Rust resume builder after a prompt. */
export function startAgentResume(options: AgentResumeOptions) {
  const { term, commandMarks, resume, cwd, shell, tabId, sessionNonce } = options
  if (!resume || !cwd || !shell) return () => {}

  let cancelled = false
  let sent = false
  const cancel = () => {
    cancelled = true
  }
  const inputDisposable = term.onData(cancel)
  const timeout = window.setTimeout(cancel, RESUME_TIMEOUT_MS)
  const stopPrompt = commandMarks.onPrompt((event) => {
    if (cancelled || sent || event.kind !== "prompt") return
    sent = true
    void invoke<string | null>("build_agent_resume_command", {
      agent: resume.agent,
      sessionId: resume.sessionId,
      cwd,
      shell,
    }).then((command) => {
      if (!cancelled && command) {
        invoke("write_pty", { tabId, sessionNonce, data: `${command}\r` }).catch(console.error)
      }
    })
  })

  return () => {
    cancelled = true
    window.clearTimeout(timeout)
    inputDisposable.dispose()
    stopPrompt()
  }
}
