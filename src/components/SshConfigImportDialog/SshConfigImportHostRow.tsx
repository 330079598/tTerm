import React from "react"
import { AlertTriangle, CheckCircle2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import {
  type SshConfigImportHost,
  statusForHost,
} from "@/components/SshConfigImportDialog/sshConfigImport"
import { formatForward } from "@/components/TunnelsPanel/tunnelUtils"
import { Badge } from "@/components/ui/badge"
import { Checkbox } from "@/components/ui/checkbox"
import { cn } from "@/lib/utils"

interface SshConfigImportHostRowProps {
  host: SshConfigImportHost
  checked: boolean
  onCheckedChange: (checked: boolean) => void
}

/** One host of the SSH config, with what importing it would set up. */
export const SshConfigImportHostRow: React.FC<SshConfigImportHostRowProps> = ({
  host,
  checked,
  onCheckedChange,
}) => {
  const { t } = useTranslation()
  const status = statusForHost(host)
  const details = [
    host.username ? `${host.username}@${host.host ?? host.name}` : host.host,
    host.port !== 22 ? `:${host.port}` : null,
    host.authMethod === "key" ? t("profiles.authMethodKey") : t("profiles.authMethodPassword"),
    host.agentForward ? t("ssh.agentForward") : null,
    host.jumpHosts.length > 0 ? t("jumpHost.viaCount", { count: host.jumpHosts.length }) : null,
  ].filter(Boolean)

  return (
    <div
      className={cn("grid gap-3 px-4 py-3 transition-colors", !host.skipped && "hover:bg-muted/25")}
    >
      <div className="flex items-start gap-3">
        <Checkbox
          className="mt-1"
          checked={checked}
          disabled={host.skipped}
          onCheckedChange={onCheckedChange}
          aria-label={t("sshConfigImport.selectHost", { name: host.name })}
        />
        <div className="min-w-0 flex-1">
          <div className="flex min-w-0 flex-wrap items-center gap-2">
            <span className="truncate text-sm font-medium">{host.name}</span>
            {host.existingProfileId && (
              <Badge variant="secondary">{t("sshConfigImport.exists")}</Badge>
            )}
            {status === "ready" && (
              <Badge variant="outline" className="text-success">
                <CheckCircle2 size={12} />
                {t("sshConfigImport.ready")}
              </Badge>
            )}
            {status === "warning" && (
              <Badge variant="outline" className="text-warning">
                <AlertTriangle size={12} />
                {t("sshConfigImport.warning")}
              </Badge>
            )}
            {status === "skipped" && (
              <Badge variant="outline" className="text-muted-foreground">
                {t("sshConfigImport.skipped")}
              </Badge>
            )}
          </div>
          <div className="text-muted-foreground mt-1 truncate text-xs">
            {details.length > 0 ? details.join(" ") : t("profiles.connectionDetailsUnavailable")}
          </div>
        </div>
      </div>

      {host.forwards.length > 0 && (
        <div className="text-muted-foreground ml-7 grid gap-0.5 font-mono text-xs">
          {host.forwards.map((forward) => (
            <div key={formatForward(forward)}>{formatForward(forward)}</div>
          ))}
        </div>
      )}

      {(host.skipReason || host.warnings.length > 0 || host.unsupportedOptions.length > 0) && (
        <div className="text-muted-foreground ml-7 grid gap-1 text-xs">
          {host.skipReason && <div>{host.skipReason}</div>}
          {host.warnings.map((warning) => (
            <div key={warning}>{warning}</div>
          ))}
          {host.unsupportedOptions.length > 0 && (
            <div>
              {t("sshConfigImport.unsupportedOptions", {
                options: host.unsupportedOptions.join(", "),
              })}
            </div>
          )}
        </div>
      )}
    </div>
  )
}
