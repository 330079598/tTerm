import React, { useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { open as openFileDialog, save as saveFileDialog } from "@tauri-apps/plugin-dialog"
import { FileDown, FileUp } from "lucide-react"
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

interface SerialFileTransferProps {
  tabId: string
  sessionNonce: number
}

/**
 * Sends files to the device, or receives what it sends, over XMODEM or
 * YMODEM, the way bootloaders take and give firmware and logs. The transfer
 * shows in the transfer list, which can cancel it.
 */
export const SerialFileTransfer: React.FC<SerialFileTransferProps> = ({ tabId, sessionNonce }) => {
  const { t } = useTranslation()
  const [protocol, setProtocol] = useState<XmodemProtocol>("ymodem")
  const protocolLabel = PROTOCOLS.find((item) => item.value === protocol)?.label

  const run = async (command: string, args: Record<string, unknown>, failure: string) => {
    try {
      await invoke(command, { tabId, sessionNonce, protocol, ...args })
    } catch (error) {
      toast({ variant: "destructive", title: failure, description: String(error) })
    }
  }

  const sendFiles = async () => {
    const selection = await openFileDialog({
      directory: false,
      multiple: protocol === "ymodem",
      title: t("serialSend.fileDialogTitle", { protocol: protocolLabel }),
    }).catch(() => null)
    const localPaths = selection ? (Array.isArray(selection) ? selection : [selection]) : []
    if (localPaths.length === 0) return
    await run("serial_send_files", { localPaths }, t("serialSend.fileFailed"))
  }

  // YMODEM names its files, so it saves into a folder; XMODEM's one file
  // is saved where asked.
  const receiveFiles = async () => {
    const title = t("serialSend.receiveDialogTitle", { protocol: protocolLabel })
    const destination =
      protocol === "ymodem"
        ? await openFileDialog({ directory: true, multiple: false, title }).catch(() => null)
        : await saveFileDialog({ title, defaultPath: "received.bin" }).catch(() => null)
    if (typeof destination !== "string" || !destination) return
    await run("serial_receive_files", { destination }, t("serialSend.receiveFailed"))
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
      <Button
        type="button"
        size="xs"
        variant="outline"
        onClick={() => void receiveFiles()}
        title={t("serialSend.receiveHint")}
      >
        <FileDown />
        {t("serialSend.receive")}
      </Button>
    </div>
  )
}
