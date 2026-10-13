import React from "react"
import { useTranslation } from "react-i18next"

import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { useConfig } from "@/contexts/ConfigContext"
import { useProxyServers } from "@/hooks/useProxyServers"
import { parseProxyChoiceKey, proxyAddress, proxyChoiceKey, proxyKindLabel } from "@/lib/proxy"
import type { ProxyServer } from "@/types/proxy"

import { ConnectionForm } from "@/components/ConnectionDialog/types"

interface ProxyFieldsProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

function serverLabel(server: ProxyServer): string {
  return `${server.name} (${proxyKindLabel(server.kind)} ${proxyAddress(server)})`
}

/** The proxy a host connects through: the default one, none, or a saved server. */
export const ProxyFields: React.FC<ProxyFieldsProps> = ({ form, setForm }) => {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { servers, loaded } = useProxyServers()

  const defaultServer = servers.find((server) => server.id === config.default_proxy_id)
  const choice = form.proxy
  const missingServer =
    loaded && choice.mode === "server" && !servers.some((server) => server.id === choice.id)

  const hint =
    choice.mode === "default"
      ? defaultServer
        ? t("proxies.field.defaultHint", { name: defaultServer.name })
        : config.default_proxy_id && loaded
          ? t("proxies.field.defaultMissingHint")
          : t("proxies.field.noDefaultHint")
      : t("proxies.field.firstHopHint")

  return (
    <div className="space-y-1.5 rounded-md border px-3 py-2">
      <Label htmlFor="conn-proxy" className="block">
        {t("proxies.field.label")}
      </Label>
      <Select
        id="conn-proxy"
        value={proxyChoiceKey(choice)}
        onChange={(event) =>
          setForm((current) => ({ ...current, proxy: parseProxyChoiceKey(event.target.value) }))
        }
      >
        <option value="default">
          {defaultServer
            ? t("proxies.field.useDefaultNamed", { name: defaultServer.name })
            : t("proxies.field.useDefault")}
        </option>
        <option value="direct">{t("proxies.field.direct")}</option>
        {servers.map((server) => (
          <option key={server.id} value={proxyChoiceKey({ mode: "server", id: server.id })}>
            {serverLabel(server)}
          </option>
        ))}
        {missingServer && (
          <option value={proxyChoiceKey(choice)}>{t("proxies.field.deletedOption")}</option>
        )}
      </Select>
      {missingServer ? (
        <p className="text-destructive text-xs" role="alert">
          {t("proxies.field.deletedHint")}
        </p>
      ) : (
        <p className="text-muted-foreground text-xs">{hint}</p>
      )}
      {servers.length === 0 && loaded && (
        <p className="text-muted-foreground text-xs">{t("proxies.field.emptyHint")}</p>
      )}
    </div>
  )
}
