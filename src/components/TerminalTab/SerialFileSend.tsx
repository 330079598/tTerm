import React, { useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { FileUp } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Select } from "@/components/ui/select"
import { toast } from "@/hooks/use-toast"

type XmodemProtocol = "xmodem" | "xmodem1k" | "ymodem"

const PROTOCOLS: { value: XmodemProtocol; label: string }[] = [
  { value: "xmodem", label: "XMODEM" },
  { value: "xmodem1k", label: "XMODEM-1K" },
  { value: "ymodem", label: "YMODEM" },
]

interface SerialFileSendProps {
  tabId: string
  sessionNonce: number
}

/**
 * Sends files to the device over XMODEM or YMODEM, the way bootloaders take
 * firmware; the transfer shows in the transfer list, which can cancel it.
 */
export const SerialFileSend: React.FC<SerialFileSendProps> = ({ tabId, sessionNonce }) => {
  const { t } = useTranslation()
  const [protocol, setProtocol] = useState<XmodemProtocol>("ymodem")

  const sendFiles = async () => {
    const selection = await openFileDialog({
      directory: false,
      multiple: protocol === "ymodem",
      title: t("serialSend.fileDialogTitle", {
        protocol: PROTOCOLS.find((item) => item.value === protocol)?.label,
      }),
    }).catch(() => null)
    const localPaths = selection ? (Array.isArray(selection) ? selection : [selection]) : []
    if (localPaths.length === 0) return
    try {
      await invoke("serial_send_files", { tabId, sessionNonce, protocol, localPaths })
    } catch (error) {
      toast({
        variant: "destructive",
        title: t("serialSend.fileFailed"),
        description: String(error),
      })
    }
  }

  return (
    <div className="flex items-center gap-1.5">
      <Select
        aria-label={t("serialSend.protocol")}
        className="h-7 w-28 text-xs"
        value={protocol}
        onChange={(event) => setProtocol(event.target.value as XmodemProtocol)}
      >
        {PROTOCOLS.map(({ value, label }) => (
          <option key={value} value={value}>
            {label}
          </option>
        ))}
      </Select>
      <Button
        type="button"
        size="xs"
        variant="outline"
        onClick={() => void sendFiles()}
        title={t("serialSend.fileHint")}
      >
        <FileUp />
        {t("serialSend.file")}
      </Button>
    </div>
  )
}
