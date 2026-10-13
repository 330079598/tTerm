import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { SquareTerminal } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"

/** A proxy command that has to be allowed on this device before it first runs. */
interface ProxyCommandPromptPayload {
  /** The command's SHA-256; connections waiting on one command share it. */
  requestId: string
  proxyName: string
  command: string
}

function withoutRequest(queue: ProxyCommandPromptPayload[], requestId: string) {
  return queue.filter((prompt) => prompt.requestId !== requestId)
}

/**
 * Asks before a proxy command from sync, a backup or an SSH config import
 * first runs here. Mounted once at the app level, because terminals, SFTP,
 * tunnels and monitoring all connect through proxies.
 */
export const ProxyCommandPrompt: React.FC = () => {
  const { t } = useTranslation()
  const [queue, setQueue] = useState<ProxyCommandPromptPayload[]>([])

  useEffect(() => {
    let disposed = false
    const unlisteners: UnlistenFn[] = []
    const register = (promise: Promise<UnlistenFn>) => {
      void promise
        .then((unlisten) => {
          if (disposed) unlisten()
          else unlisteners.push(unlisten)
        })
        .catch((error) => console.error("Failed to listen for proxy command prompts:", error))
    }
    const enqueue = (prompt: ProxyCommandPromptPayload) =>
      setQueue((current) =>
        current.some((item) => item.requestId === prompt.requestId) ? current : [...current, prompt]
      )

    register(
      listen<ProxyCommandPromptPayload>("proxy-command-prompt", (event) => enqueue(event.payload))
    )
    // The backend gave up waiting; answering now would go nowhere.
    register(
      listen<string>("proxy-command-prompt-expired", (event) =>
        setQueue((current) => withoutRequest(current, event.payload))
      )
    )
    // A tunnel that starts with the app may ask before this mounts.
    invoke<ProxyCommandPromptPayload[]>("pending_proxy_command_prompts")
      .then((pending) => {
        if (!disposed) pending.forEach(enqueue)
      })
      .catch((error) => console.error("Failed to read pending proxy command prompts:", error))

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [])

  const prompt = queue[0]
  if (!prompt) return null

  const respond = (allow: boolean) => {
    setQueue((current) => withoutRequest(current, prompt.requestId))
    invoke("respond_proxy_command_prompt", { requestId: prompt.requestId, allow }).catch((error) =>
      console.error("Failed to answer the proxy command prompt:", error)
    )
  }

  return (
    // Escape and the close button refuse, so nothing runs unless allowed.
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) respond(false)
      }}
    >
      <DialogContent
        key={prompt.requestId}
        className="sm:max-w-[520px]"
        onInteractOutside={(event) => event.preventDefault()}
      >
        <div className="space-y-4">
          <div className="space-y-1.5">
            <DialogTitle className="flex items-center gap-2 text-base">
              <SquareTerminal size={16} />
              {t("proxies.commandPrompt.title")}
            </DialogTitle>
            <DialogDescription className="text-xs leading-5">
              {t("proxies.commandPrompt.description", { name: prompt.proxyName })}
            </DialogDescription>
          </div>
          <pre className="bg-muted max-h-40 overflow-auto rounded-md px-3 py-2 font-mono text-xs leading-5 break-all whitespace-pre-wrap select-text">
            {prompt.command}
          </pre>
          <p className="text-muted-foreground text-xs leading-5">
            {t("proxies.commandPrompt.remember")}
          </p>
          <div className="flex justify-end gap-2">
            <Button variant="outline" onClick={() => respond(false)}>
              {t("proxies.commandPrompt.deny")}
            </Button>
            <Button onClick={() => respond(true)}>{t("proxies.commandPrompt.allow")}</Button>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  )
}
