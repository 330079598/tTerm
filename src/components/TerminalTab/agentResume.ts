import { invoke } from "@tauri-apps/api/core"
import type { Terminal } from "@xterm/xterm"

import type { CommandMarks } from "@/components/TerminalTab/commandMarks"
import type { AgentResumeInfo } from "@/types/tab"

export type AgentResumeNotice = {
  reason: "timeout" | "unknownShell" | "unavailable" | "unsafe" | "failed"
  enabled: boolean
  resume: () => void
}

interface AgentResumeOptions {
  term: Pick<Terminal, "onData" | "onKey">
  commandMarks: Pick<CommandMarks, "onPrompt" | "promptState">
  resume: AgentResumeInfo | undefined
  cwd: string | undefined
  tabId: string
  sessionNonce: number
  canSend: () => boolean
  onNotice: (notice: AgentResumeNotice | null) => void
}

/** Waits for a live prompt; manual and automatic sends use the Rust builder. */
export function startAgentResume(options: AgentResumeOptions) {
  const { term, commandMarks, resume, cwd, tabId, sessionNonce, onNotice, canSend } = options
  let stopped = !resume
  let ready = false
  let shell: string | undefined
  let automatic = true
  let inFlight = false
  let generation = 0
  let timer: ReturnType<typeof setTimeout> | undefined

  const cancel = () => {
    stopped = true
    generation += 1
    clearTimeout(timer)
    onNotice(null)
  }
  const manual = (reason: AgentResumeNotice["reason"], enabled = true) => {
    automatic = false
    clearTimeout(timer)
    if (!stopped) onNotice({ reason, enabled, resume: () => void send(true) })
  }
  const send = async (byButton: boolean) => {
    if (stopped || !ready || !shell || !resume || inFlight || !canSend()) return
    if (!byButton && (!automatic || commandMarks.promptState() !== "prompt")) return
    const attempt = generation
    inFlight = true
    try {
      // Saved values are untrusted, including on the manual button path.
      const command = await invoke<string | null>("build_agent_resume_command", {
        agent: resume.agent,
        sessionId: resume.sessionId,
        cwd: cwd ?? "",
        shell,
      })
      if (stopped || attempt !== generation || !canSend()) return
      if (!command) return manual("unsafe", false)
      if (!byButton && (!automatic || commandMarks.promptState() !== "prompt")) return
      await invoke("write_pty", { tabId, sessionNonce, data: `${command}\r` })
      cancel()
    } catch (error) {
      console.error(error)
      if (!stopped) manual("failed")
    } finally {
      inFlight = false
    }
  }
  const input = term.onData((data) => {
    // Terminal cursor/focus replies are not user input. Keys and pastes are.
    if (!data.startsWith("\x1b") || data.startsWith("\x1b[200~")) cancel()
  })
  const keys = term.onKey(cancel)
  const stopPrompt = commandMarks.onPrompt((event) => {
    if (event.kind === "prompt") void send(false)
  })

  return {
    /** Called only after this connection authenticated and opened its shell. */
    ready(detectedShell: string | undefined, integration: boolean) {
      if (stopped) return
      generation += 1
      ready = true
      shell = detectedShell
      if (!shell) return manual("unknownShell", false)
      if (!integration || shell.toLowerCase() === "cmd") return manual("unavailable")
      timer = setTimeout(() => manual("timeout"), 15_000)
      void send(false)
    },
    disconnect: cancel,
    dispose() {
      cancel()
      input.dispose()
      keys.dispose()
      stopPrompt()
    },
  }
}

export type AgentResumeController = ReturnType<typeof startAgentResume>
