import React, { useState } from "react"
import { Send, Square } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import {
  MIN_REPEAT_MS,
  type SerialSendMode,
  useSerialSender,
} from "@/components/TerminalTab/useSerialSender"
import type { SerialLineEnding } from "@/types/serial"

interface SerialSendBarProps {
  tabId: string
  sessionNonce: number
  lineEnding: SerialLineEnding
}

/** A serial tab's send bar: text or hex to send once, or over and over. */
export const SerialSendBar: React.FC<SerialSendBarProps> = ({
  tabId,
  sessionNonce,
  lineEnding,
}) => {
  const { t } = useTranslation()
  const { history, repeating, send, stopRepeat } = useSerialSender({
    tabId,
    sessionNonce,
    lineEnding,
  })
  const [mode, setMode] = useState<SerialSendMode>("text")
  const [text, setText] = useState("")
  const [appendEnter, setAppendEnter] = useState(true)
  const [repeat, setRepeat] = useState(false)
  const [repeatMs, setRepeatMs] = useState(1000)
  // Where ArrowUp has got to in the history; -1 is the text being typed.
  const [historyIndex, setHistoryIndex] = useState(-1)

  const submit = () => {
    setHistoryIndex(-1)
    void send({ mode, text }, appendEnter, repeat ? repeatMs : undefined)
  }

  const recall = (index: number) => {
    const entry = history[index]
    setHistoryIndex(index)
    if (!entry) {
      setText("")
      return
    }
    setMode(entry.mode)
    setText(entry.text)
  }

  return (
    <div className="border-border bg-muted/30 flex flex-wrap items-center gap-2 border-b px-3 py-1.5 text-xs">
      <Select
        aria-label={t("serialSend.mode")}
        className="h-7 w-20 text-xs"
        value={mode}
        onChange={(event) => setMode(event.target.value as SerialSendMode)}
      >
        <option value="text">{t("serialSend.text")}</option>
        <option value="hex">HEX</option>
      </Select>
      <Input
        aria-label={t("serialSend.input")}
        className="h-7 min-w-40 flex-1 font-mono text-xs"
        value={text}
        placeholder={mode === "hex" ? "AA 55 01 0D" : t("serialSend.textPlaceholder")}
        onChange={(event) => {
          setText(event.target.value)
          setHistoryIndex(-1)
        }}
        onKeyDown={(event) => {
          if (event.key === "Enter") {
            event.preventDefault()
            submit()
          } else if (event.key === "ArrowUp" && historyIndex + 1 < history.length) {
            event.preventDefault()
            recall(historyIndex + 1)
          } else if (event.key === "ArrowDown" && historyIndex >= 0) {
            event.preventDefault()
            recall(historyIndex - 1)
          }
        }}
      />
      <div className="flex items-center gap-1.5">
        <Checkbox
          id={`serial-send-enter-${tabId}`}
          checked={appendEnter}
          onCheckedChange={setAppendEnter}
        />
        <Label htmlFor={`serial-send-enter-${tabId}`} className="text-xs font-normal">
          {t("serialSend.appendEnter")}
        </Label>
      </div>
      <div className="flex items-center gap-1.5">
        <Checkbox
          id={`serial-send-repeat-${tabId}`}
          checked={repeat}
          onCheckedChange={(checked) => {
            setRepeat(checked)
            if (!checked) stopRepeat()
          }}
        />
        <Label htmlFor={`serial-send-repeat-${tabId}`} className="text-xs font-normal">
          {t("serialSend.repeat")}
        </Label>
        <Input
          aria-label={t("serialSend.repeatInterval")}
          type="number"
          min={MIN_REPEAT_MS}
          className="h-7 w-20 text-xs"
          value={repeatMs}
          disabled={!repeat}
          onChange={(event) => setRepeatMs(Number(event.target.value))}
        />
        <span className="text-muted-foreground">ms</span>
      </div>
      {repeating ? (
        <Button type="button" size="xs" variant="outline" onClick={stopRepeat}>
          <Square />
          {t("serialSend.stop")}
        </Button>
      ) : (
        <Button type="button" size="xs" onClick={submit}>
          <Send />
          {t("serialSend.send")}
        </Button>
      )}
    </div>
  )
}
