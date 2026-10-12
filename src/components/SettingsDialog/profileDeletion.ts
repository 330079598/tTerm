import type { TFunction } from "i18next"

import { summarizeTunnelNames } from "@/components/TunnelsPanel/tunnelUtils"

/**
 * The paragraph a delete confirmation adds when tunnels go through the hosts
 * being deleted, or an empty string when none do.
 */
export function profileTunnelNote(
  t: TFunction,
  tunnels: Parameters<typeof summarizeTunnelNames>[0]
) {
  return tunnels.length > 0
    ? `\n\n${t("tunnels.profileInUse", {
        count: tunnels.length,
        names: summarizeTunnelNames(tunnels),
        defaultValue:
          "{{count}} port-forwarding tunnel(s) use this host and will be stopped: {{names}}. Their rules are kept; choose another host for them afterwards.",
      })}`
    : ""
}
