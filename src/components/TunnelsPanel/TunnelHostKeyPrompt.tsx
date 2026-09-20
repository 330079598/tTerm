import React, { useCallback, useEffect, useState } from "react"
import { listen } from "@tauri-apps/api/event"

import { HostKeyPromptDialog } from "@/components/TerminalTab/HostKeyPromptDialog"
import type { HostKeyPromptState } from "@/components/TerminalTab/types"

import "@/components/TerminalTab.css"

/**
 * Asks the user to confirm SSH host fingerprints for tunnels. Mounted once at
 * the app level so a prompt is visible wherever the user is (tunnels can start
 * on launch, before the Port Forwarding page is ever opened). Prompts from
 * several tunnels are answered one at a time.
 */
export const TunnelHostKeyPrompt: React.FC = () => {
  const [queue, setQueue] = useState<HostKeyPromptState[]>([])

  useEffect(() => {
    let disposed = false
    let unlisten: (() => void) | null = null
    listen<HostKeyPromptState>("ssh-hostkey-prompt-tunnels", (event) =>
      setQueue((current) => [...current, event.payload])
    )
      .then((off) => {
        if (disposed) off()
        else unlisten = off
      })
      .catch((error) => console.error("Failed to listen for tunnel host key prompts:", error))
    return () => {
      disposed = true
      unlisten?.()
    }
  }, [])

  const handleChange = useCallback((value: HostKeyPromptState | null) => {
    // The dialog reports null once the current prompt has been answered.
    if (value === null) setQueue((current) => current.slice(1))
  }, [])

  return <HostKeyPromptDialog hostKeyPrompt={queue[0] ?? null} setHostKeyPrompt={handleChange} />
}
