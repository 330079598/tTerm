import React, { useEffect, useRef, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen, type UnlistenFn } from "@tauri-apps/api/event"
import { KeyRound } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"

/** A question the SSH server (or tTerm, for a missing password) has for the user. */
export interface SshAuthPrompt {
  requestId: string
  host: string
  port: number
  username: string
  hopIndex?: number
  totalHops?: number
  kind: "password" | "keyboard-interactive"
  name: string
  instructions: string
  prompts: { text: string; echo: boolean }[]
}

interface SshAuthPromptDialogProps {
  /** The tab (or test connection) whose sessions may ask. */
  tabId: string
}

/**
 * Shows the authentication prompts of one tab's SSH sessions, one at a time,
 * over the tab's content. Listens on its own so every place that opens a
 * session for a tab id only has to mount it.
 */
export const SshAuthPromptDialog: React.FC<SshAuthPromptDialogProps> = ({ tabId }) => {
  const [queue, setQueue] = useState<SshAuthPrompt[]>([])

  useEffect(() => {
    let disposed = false
    const unlisteners: UnlistenFn[] = []
    const register = (promise: Promise<UnlistenFn>) => {
      void promise.then((unlisten) => {
        if (disposed) unlisten()
        else unlisteners.push(unlisten)
      })
    }

    register(
      listen<SshAuthPrompt>(`ssh-auth-prompt-${tabId}`, (event) => {
        setQueue((current) => [...current, event.payload])
      })
    )
    // The backend gave up waiting; answering now would go nowhere.
    register(
      listen<string>(`ssh-auth-prompt-expired-${tabId}`, (event) => {
        setQueue((current) => current.filter((prompt) => prompt.requestId !== event.payload))
      })
    )

    return () => {
      disposed = true
      unlisteners.forEach((unlisten) => unlisten())
    }
  }, [tabId])

  const prompt = queue[0]
  if (!prompt) return null

  const respond = async (responses: string[] | null) => {
    setQueue((current) => current.filter((item) => item.requestId !== prompt.requestId))
    await invoke("respond_ssh_auth_prompt", { requestId: prompt.requestId, responses }).catch(
      console.error
    )
  }

  return <SshAuthPromptForm key={prompt.requestId} prompt={prompt} onRespond={respond} />
}

const SshAuthPromptForm: React.FC<{
  prompt: SshAuthPrompt
  onRespond: (responses: string[] | null) => Promise<void>
}> = ({ prompt, onRespond }) => {
  const { t } = useTranslation()
  const [answers, setAnswers] = useState(() => prompt.prompts.map(() => ""))
  const firstInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    // The terminal holds the focus while connecting.
    requestAnimationFrame(() => firstInputRef.current?.focus())
  }, [])

  const endpoint = `${prompt.username}@${prompt.host}:${prompt.port}`

  return (
    <div className="host-key-dialog-overlay">
      <form
        className="host-key-dialog-content space-y-4"
        role="dialog"
        aria-label={t("ssh.authPrompt.title")}
        onSubmit={(event) => {
          event.preventDefault()
          void onRespond(answers)
        }}
        onKeyDown={(event) => {
          if (event.key === "Escape") {
            event.preventDefault()
            void onRespond(null)
          }
        }}
      >
        <div>
          <h3 className="flex items-center gap-2 text-base font-semibold">
            <KeyRound size={16} />
            {t("ssh.authPrompt.title")}
          </h3>
          <p className="text-muted-foreground mt-1 font-mono text-xs break-all">
            {prompt.hopIndex
              ? t("ssh.authPrompt.jumpHost", { index: prompt.hopIndex, endpoint })
              : endpoint}
          </p>
        </div>

        {(prompt.name || prompt.instructions) && (
          <div className="bg-muted/50 rounded-md px-3 py-2 text-xs leading-5">
            {prompt.name && <div className="font-medium">{prompt.name}</div>}
            {prompt.instructions && (
              <div className="text-muted-foreground whitespace-pre-line">{prompt.instructions}</div>
            )}
          </div>
        )}

        {prompt.prompts.map((field, index) => {
          const id = `ssh-auth-prompt-${prompt.requestId}-${index}`
          return (
            <div key={id} className="space-y-1.5">
              <Label htmlFor={id}>
                {prompt.kind === "password"
                  ? t("ssh.authPrompt.password")
                  : field.text.trim() || t("ssh.authPrompt.response")}
              </Label>
              <Input
                ref={index === 0 ? firstInputRef : undefined}
                id={id}
                type={field.echo ? "text" : "password"}
                autoComplete="off"
                autoCapitalize="none"
                autoCorrect="off"
                spellCheck={false}
                value={answers[index] ?? ""}
                onChange={(event) =>
                  setAnswers((current) =>
                    current.map((value, position) =>
                      position === index ? event.target.value : value
                    )
                  )
                }
              />
            </div>
          )
        })}

        <p className="text-muted-foreground text-xs leading-5">{t("ssh.authPrompt.hint")}</p>

        <div className="flex justify-end gap-2">
          <Button type="button" variant="outline" onClick={() => void onRespond(null)}>
            {t("common.cancel")}
          </Button>
          <Button type="submit">{t("ssh.authPrompt.submit")}</Button>
        </div>
      </form>
    </div>
  )
}
