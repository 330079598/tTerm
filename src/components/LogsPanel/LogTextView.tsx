import React, { useEffect, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { CodeMirrorEditor } from "@/components/CodeMirrorEditor"
import { toErrorMessage } from "@/lib/utils"

export const LogTextView: React.FC<{ sessionId: string }> = ({ sessionId }) => {
  const [text, setText] = useState<string | null>(null)
  const [error, setError] = useState<string | null>(null)
  const { t } = useTranslation()

  // The panel remounts the view for another session, so this loads once.
  useEffect(() => {
    let disposed = false
    invoke<ArrayBuffer>("load_terminal_log_text", { id: sessionId })
      .then((buffer) => {
        if (!disposed) setText(new TextDecoder().decode(buffer))
      })
      .catch((reason) => {
        if (!disposed) setError(toErrorMessage(reason))
      })
    return () => {
      disposed = true
    }
  }, [sessionId])

  if (error) {
    return (
      <div role="alert" className="text-destructive p-6 text-sm">
        {error}
      </div>
    )
  }
  if (text === null) {
    return <p className="text-muted-foreground p-6 text-sm">{t("terminalLogs.loading")}</p>
  }
  return <CodeMirrorEditor className="h-full min-h-0" value={text} readOnly />
}
