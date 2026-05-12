import React from "react"
import { Check, Copy, Route, ShieldCheck } from "lucide-react"
import { invoke } from "@tauri-apps/api/core"
import { useTranslation } from "react-i18next"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogClose,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { toast } from "@/hooks/use-toast"
import type { TerminalTabProps } from "@/components/TerminalTab/types"
import type { JumpHostConnection } from "@/types/tab"

type JumpHostInfoDialogProps = {
  connection: TerminalTabProps["connection"]
  open: boolean
  onOpenChange: (open: boolean) => void
  onDontShowAgainChange: (checked: boolean) => void
  dontShowAgain: boolean
}

type JumpHostDetail = {
  label: string
  value: string
  copyValue?: string
}

const hasClipboardPluginError = (error: unknown) => {
  return String(error).toLowerCase().includes("plugin:clipboard-manager")
}

function buildJumpHostAddress(jump: Pick<JumpHostConnection, "host" | "port">) {
  return `${jump.host}:${jump.port ?? 22}`
}

function buildSshDestination(connection: NonNullable<TerminalTabProps["connection"]>) {
  const host = connection.host || "unknown-host"
  const port = connection.port ?? 22
  return connection.username ? `${connection.username}@${host}:${port}` : `${host}:${port}`
}

function buildProxyJumpValue(jumpHosts: JumpHostConnection[]) {
  return jumpHosts
    .map((jump) => `${jump.username ? `${jump.username}@` : ""}${jump.host}:${jump.port ?? 22}`)
    .join(",")
}

function buildSshCommand(connection: NonNullable<TerminalTabProps["connection"]>) {
  const host = connection.host || "unknown-host"
  const args: string[] = ["ssh"]

  if (connection.jumpHosts?.length) {
    args.push("-J", buildProxyJumpValue(connection.jumpHosts))
  }

  if (connection.port && connection.port !== 22) {
    args.push("-p", String(connection.port))
  }

  args.push(connection.username ? `${connection.username}@${host}` : host)
  return args.join(" ")
}

export const JumpHostInfoDialog: React.FC<JumpHostInfoDialogProps> = ({
  connection,
  dontShowAgain,
  onDontShowAgainChange,
  onOpenChange,
  open,
}) => {
  const { t } = useTranslation()
  const [copiedKey, setCopiedKey] = React.useState<string | null>(null)
  const jumpHosts = connection?.jumpHosts ?? []

  const details = React.useMemo<JumpHostDetail[]>(() => {
    if (!connection || jumpHosts.length === 0) return []

    return [
      {
        label: t("jumpHostInfo.target", { defaultValue: "Target" }),
        value: buildSshDestination(connection),
      },
      {
        label: t("jumpHostInfo.method", { defaultValue: "Method" }),
        value: "ProxyJump",
      },
      {
        label: t("jumpHostInfo.sshCommand", { defaultValue: "SSH command" }),
        value: buildSshCommand(connection),
      },
    ]
  }, [connection, jumpHosts.length, t])

  const copyText = React.useCallback(
    async (key: string, text: string) => {
      try {
        await invoke("plugin:clipboard-manager|write_text", { text })
        setCopiedKey(key)
        window.setTimeout(() => setCopiedKey((current) => (current === key ? null : current)), 1400)
        toast({
          title: t("jumpHostInfo.copySuccessTitle", { defaultValue: "Copied" }),
          description: t("jumpHostInfo.copySuccessDescription", {
            defaultValue: "Connection information copied to the clipboard.",
          }),
        })
      } catch (error) {
        if (!hasClipboardPluginError(error) && navigator.clipboard?.writeText) {
          try {
            await navigator.clipboard.writeText(text)
            setCopiedKey(key)
            window.setTimeout(
              () => setCopiedKey((current) => (current === key ? null : current)),
              1400
            )
            return
          } catch (clipboardError) {
            console.error("Failed to copy jump host information:", clipboardError)
          }
        } else {
          console.error("Failed to copy jump host information:", error)
        }

        toast({
          title: t("jumpHostInfo.copyFailedTitle", { defaultValue: "Copy failed" }),
          description: t("jumpHostInfo.copyFailedDescription", {
            defaultValue: "Unable to copy the connection information.",
          }),
          variant: "destructive",
        })
      }
    },
    [t]
  )

  if (!connection || jumpHosts.length === 0) {
    return null
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="jump-host-info-dialog sm:max-w-[620px]">
        <DialogHeader>
          <div className="jump-host-info-title-row">
            <span className="jump-host-info-icon" aria-hidden="true">
              <Route size={18} />
            </span>
            <div>
              <DialogTitle>
                {t("jumpHostInfo.title", { defaultValue: "Jump Host Connection" })}
              </DialogTitle>
              <DialogDescription>
                {t("jumpHostInfo.description", {
                  count: jumpHosts.length,
                  defaultValue:
                    "This session is connected through {{count}} jump host(s). Review the route before continuing.",
                })}
              </DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div
          className="jump-host-info-route"
          aria-label={t("jumpHostInfo.route", { defaultValue: "Route" })}
        >
          {jumpHosts.map((jump, index) => (
            <div className="jump-host-info-hop" key={`${jump.host}:${jump.port}:${index}`}>
              <div className="jump-host-info-hop-header">
                <Badge variant="secondary">
                  {t("jumpHostInfo.hop", { index: index + 1, defaultValue: "Jump #{{index}}" })}
                </Badge>
                <span className="jump-host-info-auth">
                  <ShieldCheck size={13} />
                  {jump.authMethod === "key"
                    ? t("jumpHostInfo.keyAuth", { defaultValue: "Key auth" })
                    : t("jumpHostInfo.passwordAuth", { defaultValue: "Password auth" })}
                </span>
              </div>
              <div className="jump-host-info-hop-address">{buildJumpHostAddress(jump)}</div>
              <div className="jump-host-info-hop-user">{jump.username}</div>
              {jump.authMethod === "key" && jump.privateKeyPath && (
                <div className="jump-host-info-key-path">
                  {t("jumpHostInfo.privateKey", { defaultValue: "Private key" })}:{" "}
                  {jump.privateKeyPath}
                </div>
              )}
            </div>
          ))}
        </div>

        <div className="jump-host-info-details">
          {details.map((detail) => {
            const copyValue = detail.copyValue ?? detail.value
            const copied = copiedKey === detail.label

            return (
              <div className="jump-host-info-detail-row" key={detail.label}>
                <div className="jump-host-info-detail-copy">
                  <span className="jump-host-info-detail-label">{detail.label}</span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon-xs"
                    aria-label={t("jumpHostInfo.copyField", {
                      label: detail.label,
                      defaultValue: "Copy {{label}}",
                    })}
                    onClick={() => void copyText(detail.label, copyValue)}
                  >
                    {copied ? <Check size={13} /> : <Copy size={13} />}
                  </Button>
                </div>
                <code className="jump-host-info-detail-value">{detail.value}</code>
              </div>
            )
          })}
        </div>

        <label className="jump-host-info-checkbox">
          <Checkbox checked={dontShowAgain} onCheckedChange={onDontShowAgainChange} />
          <span>{t("jumpHostInfo.dontShowAgain", { defaultValue: "Do not show this again" })}</span>
        </label>

        <DialogFooter>
          <Button
            type="button"
            variant="outline"
            onClick={() => void copyText("command", buildSshCommand(connection))}
          >
            <Copy size={14} />
            {t("jumpHostInfo.copyCommand", { defaultValue: "Copy command" })}
          </Button>
          <DialogClose asChild>
            <Button type="button">{t("common.close", { defaultValue: "Close" })}</Button>
          </DialogClose>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  )
}
