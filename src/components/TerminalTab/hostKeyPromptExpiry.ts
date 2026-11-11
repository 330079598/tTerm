import type { Dispatch, SetStateAction } from "react"
import { listen } from "@tauri-apps/api/event"
import type { HostKeyPromptState } from "@/components/TerminalTab/types"

/** A late expiry must never dismiss a newer question on the same tab. */
export function listenForHostKeyPromptExpiry(
  tabId: string,
  setPrompt: Dispatch<SetStateAction<HostKeyPromptState | null>>
) {
  return listen<string>(`ssh-hostkey-prompt-expired-${tabId}`, (event) => {
    setPrompt((current) => (current?.requestId === event.payload ? null : current))
  })
}
