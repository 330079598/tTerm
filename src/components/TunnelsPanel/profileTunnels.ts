import { invoke } from "@tauri-apps/api/core"

import { isTunnelActive } from "@/components/TunnelsPanel/tunnelUtils"
import type { TunnelRule, TunnelStatus } from "@/types/tunnel"

/** Tunnel rules that connect through a saved host. Failing to look is not worth blocking the caller. */
export async function findTunnelsUsingProfile(profileId: string): Promise<TunnelRule[]> {
  try {
    const rules = await invoke<TunnelRule[]>("list_tunnels")
    return rules.filter((rule) => rule.profileId === profileId)
  } catch (error) {
    console.error("Failed to look up tunnels for a host:", error)
    return []
  }
}

/** The subset of those that is running (or starting/reconnecting) right now. */
export async function findActiveTunnelsUsingProfile(profileId: string): Promise<TunnelRule[]> {
  const rules = await findTunnelsUsingProfile(profileId)
  if (rules.length === 0) return []
  try {
    const statuses = await invoke<TunnelStatus[]>("list_tunnel_statuses")
    const active = new Set(
      statuses.filter((status) => isTunnelActive(status.state)).map((status) => status.id)
    )
    return rules.filter((rule) => active.has(rule.id))
  } catch (error) {
    console.error("Failed to read tunnel statuses:", error)
    return []
  }
}
