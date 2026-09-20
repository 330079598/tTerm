import React from "react"
import { Laptop, Server, Target } from "lucide-react"
import { useTranslation } from "react-i18next"

import { cn } from "@/lib/utils"
import type { TunnelKind, TunnelState } from "@/types/tunnel"
import {
  getRouteNodes,
  type RouteNode,
  type RouteNodeKind,
} from "@/components/TunnelsPanel/tunnelUtils"

import "@/components/TunnelsPanel/TunnelsPanel.css"

const NODE_ICONS: Record<RouteNodeKind, React.ComponentType<{ className?: string }>> = {
  device: Laptop,
  server: Server,
  target: Target,
}

type FlowState = "active" | "pending" | "error" | "idle"

/** Lets long endpoints wrap after `.` or `:` instead of mid-number. */
function breakable(value: string): React.ReactNode {
  return value.split(/(?<=[.:])/).map((part, index) => (
    <React.Fragment key={index}>
      {index > 0 && <wbr />}
      {part}
    </React.Fragment>
  ))
}

function flowFor(state: TunnelState | undefined): FlowState {
  switch (state) {
    case "running":
      return "active"
    case "starting":
    case "reconnecting":
      return "pending"
    case "error":
      return "error"
    default:
      return "idle"
  }
}

interface TunnelRouteProps {
  kind: TunnelKind
  bindHost: string
  bindPort: number
  destHost: string
  destPort: number
  hostLabel: string
  state?: TunnelState
  className?: string
}

const RouteNodeCard: React.FC<{ node: RouteNode; kind: TunnelKind; dimmed: boolean }> = ({
  node,
  kind,
  dimmed,
}) => {
  const { t } = useTranslation()
  const Icon = NODE_ICONS[node.kind]
  const title =
    node.title ||
    (node.kind === "device"
      ? kind === "dynamic"
        ? t("tunnels.route.proxy", { defaultValue: "SOCKS5 proxy" })
        : t("tunnels.route.device", { defaultValue: "This device" })
      : node.kind === "server"
        ? t("tunnels.route.server", { defaultValue: "SSH host" })
        : t("tunnels.route.target", { defaultValue: "Destination" }))
  const detail =
    node.detail ??
    (node.kind === "target" && kind === "dynamic"
      ? t("tunnels.route.anyDestination", { defaultValue: "Any address" })
      : undefined)

  return (
    <div
      className={cn(
        "bg-background flex min-w-0 flex-1 basis-0 flex-col items-center gap-1 rounded-md border px-2 py-2 text-center transition-opacity",
        dimmed && "opacity-60"
      )}
    >
      <Icon className="text-muted-foreground size-4" />
      <div className="w-full truncate text-xs font-medium" title={title}>
        {title}
      </div>
      <div
        className="text-muted-foreground line-clamp-2 min-h-4 w-full font-mono text-[11px] leading-snug [overflow-wrap:anywhere]"
        title={detail}
      >
        {detail ? breakable(detail) : ""}
      </div>
    </div>
  )
}

export const TunnelRoute: React.FC<TunnelRouteProps> = ({
  kind,
  bindHost,
  bindPort,
  destHost,
  destPort,
  hostLabel,
  state,
  className,
}) => {
  const nodes = getRouteNodes({ kind, bindHost, bindPort, destHost, destPort }, hostLabel)
  const flow = flowFor(state)

  return (
    <div
      className={cn("flex items-center", className)}
      role="img"
      aria-label={nodes
        .map((node) => [node.title, node.detail].filter(Boolean).join(" "))
        .join(" → ")}
    >
      {nodes.map((node, index) => (
        <React.Fragment key={index}>
          {index > 0 && <div className="tunnel-connector" data-flow={flow} aria-hidden="true" />}
          <RouteNodeCard node={node} kind={kind} dimmed={flow === "idle"} />
        </React.Fragment>
      ))}
    </div>
  )
}
