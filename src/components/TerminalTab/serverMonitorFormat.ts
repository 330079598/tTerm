import type { ServerMetricsSnapshot } from "@/components/TerminalTab/serverMetrics"

const DISTRO_ICON_BASE = "/assets/distro-icons"

export function formatKib(value?: number) {
  if (value === undefined) return "--"
  const bytes = value * 1024
  const units = ["B", "KB", "MB", "GB", "TB"]
  let current = bytes
  let unitIndex = 0
  while (current >= 1024 && unitIndex < units.length - 1) {
    current /= 1024
    unitIndex += 1
  }
  const digits = current >= 10 || unitIndex < 2 ? 0 : 1
  return `${current.toFixed(digits)}${units[unitIndex]}`
}

export function formatKibAsG(value?: number) {
  if (value === undefined) return "--"
  const gib = value / 1024 / 1024
  const digits = gib >= 10 ? 0 : 1
  return `${gib.toFixed(digits)}G`
}

export function severityClass(percent?: number) {
  if (percent === undefined) return "is-muted"
  if (percent >= 90) return "is-critical"
  if (percent >= 70) return "is-warning"
  return "is-normal"
}

export function latencySeverityClass(latencyMs?: number) {
  if (latencyMs === undefined) return "is-muted"
  if (latencyMs >= 300) return "is-critical"
  if (latencyMs >= 150) return "is-warning"
  return "is-normal"
}

export function formatLatency(latencyMs?: number) {
  if (latencyMs === undefined) return "--"
  return latencyMs === 0 ? "<1 ms" : `${latencyMs} ms`
}

export function formatNetworkRate(value?: number) {
  if (value === undefined || !Number.isFinite(value)) return "--"
  const units = ["B", "K", "M", "G"]
  let current = Math.max(0, value)
  let index = 0
  while (current >= 1024 && index < units.length - 1) {
    current /= 1024
    index += 1
  }
  const digits = current >= 100 || index === 0 ? 0 : current >= 10 ? 1 : 2
  return `${current.toFixed(digits)}${units[index]}/s`
}

export function formatUptime(seconds?: number) {
  if (seconds === undefined) return "--"
  const days = Math.floor(seconds / 86400)
  const hours = Math.floor((seconds % 86400) / 3600)
  return days > 0 ? `${days}d ${hours}h` : `${hours}h`
}

const DISTRO_RULES: Array<{ patterns: string[]; id: string; exact?: boolean }> = [
  { patterns: ["archlinux"], id: "arch" },
  { patterns: ["ubuntu"], id: "ubuntu" },
  { patterns: ["linuxmint", "linux mint"], id: "linuxmint" },
  { patterns: ["mint"], id: "linuxmint" },
  { patterns: ["debian"], id: "debian" },
  { patterns: ["gentoo"], id: "gentoo" },
  { patterns: ["manjaro"], id: "manjaro" },
  { patterns: ["kali"], id: "kali" },
  { patterns: ["pop!_os", "pop_os"], id: "popos" },
  { patterns: ["pop os", "pop!"], id: "popos" },
  { patterns: ["zorin"], id: "zorin" },
  { patterns: ["elementary"], id: "elementary" },
  { patterns: ["mx linux"], id: "mx" },
  { patterns: ["mx"], id: "mx", exact: true },
  { patterns: ["endeavouros"], id: "endeavouros" },
  { patterns: ["endeavour"], id: "endeavouros" },
  { patterns: ["cachyos"], id: "cachyos" },
  { patterns: ["nixos", "nix os"], id: "nixos" },
  { patterns: ["nobara"], id: "nobara" },
  { patterns: ["bazzite"], id: "bazzite" },
  { patterns: ["antix"], id: "antix" },
  { patterns: ["biglinux", "big linux"], id: "biglinux" },
  { patterns: ["deepin"], id: "deepin" },
  { patterns: ["garuda"], id: "garuda" },
  { patterns: ["slackware"], id: "slackware" },
  { patterns: ["void"], id: "void" },
  { patterns: ["parrot"], id: "parrot" },
  { patterns: ["rocky"], id: "rocky" },
  { patterns: ["alma"], id: "alma" },
  { patterns: ["centos"], id: "centos" },
  { patterns: ["rhel", "redhat", "red hat"], id: "rhel" },
  { patterns: ["fedora"], id: "fedora" },
  { patterns: ["suse", "opensuse"], id: "suse" },
  { patterns: ["arch"], id: "arch" },
  { patterns: ["alpine"], id: "alpine" },
  { patterns: ["amazon"], id: "amazon" },
  { patterns: ["amzn"], id: "amazon", exact: true },
  { patterns: ["oracle"], id: "oracle" },
]

export function normalizeDistroId(snapshot?: ServerMetricsSnapshot) {
  const values = [
    snapshot?.distribution?.id,
    ...(snapshot?.distribution?.idLike ?? []),
    snapshot?.distribution?.name,
    snapshot?.distribution?.prettyName,
  ]
    .filter(Boolean)
    .map((value) => value!.toLowerCase())

  for (const rule of DISTRO_RULES) {
    const matches = rule.exact
      ? values.some((value) => rule.patterns.some((pattern) => value === pattern))
      : values.some((value) => rule.patterns.some((pattern) => value.includes(pattern)))
    if (matches) {
      return rule.id
    }
  }

  return "linux"
}

export function getDistroLabel(snapshot?: ServerMetricsSnapshot) {
  if (!snapshot?.distribution) return "Linux"
  const pretty = snapshot.distribution.prettyName || snapshot.distribution.name
  return pretty || "Linux"
}

export function getDistroIconSrc(distroId: string) {
  return `${DISTRO_ICON_BASE}/${distroId}.svg`
}
