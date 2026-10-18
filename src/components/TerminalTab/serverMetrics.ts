import type { LinuxDistributionInfo } from "@/lib/distro"

const CPU_HISTORY_SAMPLE_LIMIT = 48

const CPU_SPARKLINE_SMOOTHING = 0.45

export type CpuTimes = {
  user: number
  nice: number
  system: number
  idle: number
  iowait: number
  irq: number
  softirq: number
  steal: number
  guest: number
  guestNice: number
}

export type CpuCoreTimes = {
  id: string
  times: CpuTimes
}

export type CpuHistorySample = {
  total: number
  cores: Array<{ id: string; percent: number }>
}

type MemoryMetrics = {
  totalKib: number
  availableKib: number
  usedKib: number
  usedPercent: number
  swapTotalKib: number
  swapFreeKib: number
  swapUsedKib: number
  swapUsedPercent: number
}

export type DiskMetrics = {
  filesystem: string
  filesystemType: string
  mount: string
  totalKib: number
  usedKib: number
  availableKib: number
  usedPercent: number
}

export type NetworkMetrics = {
  interface: string
  receivedBytes: number
  transmittedBytes: number
  receiveErrors: number
  transmitErrors: number
  receiveDropped: number
  transmitDropped: number
}

type LoadAverageMetrics = {
  one: number
  five: number
  fifteen: number
}

export type NetworkRate = {
  receivedBytesPerSecond: number
  transmittedBytesPerSecond: number
}

export type NetworkHistorySample = NetworkRate & {
  capturedAt: number
}

export type ServerMetricsSnapshot = {
  supported: boolean
  unsupportedReason?: string
  distribution?: LinuxDistributionInfo
  kernel?: string
  cpuTimes?: CpuTimes
  cpuCoreTimes?: CpuCoreTimes[]
  memory?: MemoryMetrics
  primaryIp?: string
  disk?: DiskMetrics
  disks?: DiskMetrics[]
  network?: NetworkMetrics
  loadAverage?: LoadAverageMetrics
  uptimeSecs?: number
  networkLatencyMs?: number
}

function cpuTotal(times: CpuTimes) {
  return (
    times.user +
    times.nice +
    times.system +
    times.idle +
    times.iowait +
    times.irq +
    times.softirq +
    times.steal
  )
}

function cpuIdle(times: CpuTimes) {
  return times.idle + times.iowait
}

export function calculateCpuPercent(previous?: CpuTimes, next?: CpuTimes) {
  if (!previous || !next) return undefined

  const totalDelta = cpuTotal(next) - cpuTotal(previous)
  const idleDelta = cpuIdle(next) - cpuIdle(previous)
  if (totalDelta <= 0) return undefined

  return Math.max(0, Math.min(100, ((totalDelta - idleDelta) / totalDelta) * 100))
}

export function normalizePercent(value: number) {
  return Math.max(0, Math.min(100, value))
}

function smoothPercent(previous: number | undefined, next: number) {
  if (previous === undefined) {
    return normalizePercent(next)
  }

  return normalizePercent(previous * CPU_SPARKLINE_SMOOTHING + next * (1 - CPU_SPARKLINE_SMOOTHING))
}

export function calculateCpuCorePercents(previous?: CpuCoreTimes[], next?: CpuCoreTimes[]) {
  if (!previous?.length || !next?.length) {
    return []
  }

  const previousById = new Map(previous.map((core) => [core.id, core.times]))
  return next
    .map((core) => ({
      id: core.id,
      percent: calculateCpuPercent(previousById.get(core.id), core.times),
    }))
    .filter((core): core is { id: string; percent: number } => core.percent !== undefined)
}

export function appendCpuHistorySample(
  history: CpuHistorySample[],
  total?: number,
  cores: Array<{ id: string; percent: number }> = []
) {
  if (total === undefined || Number.isNaN(total)) {
    return history
  }

  const previousSample = history[history.length - 1]
  const previousCoresById = new Map(
    previousSample?.cores.map((core) => [core.id, core.percent]) ?? []
  )

  return [
    ...history,
    {
      total: smoothPercent(previousSample?.total, total),
      cores: cores.map((core) => ({
        id: core.id,
        percent: smoothPercent(previousCoresById.get(core.id), core.percent),
      })),
    },
  ].slice(-CPU_HISTORY_SAMPLE_LIMIT)
}

export function calculateNetworkRate(
  previous: { metrics: NetworkMetrics; capturedAt: number } | undefined,
  next: NetworkMetrics | undefined,
  capturedAt: number
): NetworkRate | undefined {
  if (!previous || !next || previous.metrics.interface !== next.interface) return undefined
  const elapsedSeconds = (capturedAt - previous.capturedAt) / 1000
  if (
    elapsedSeconds <= 0 ||
    next.receivedBytes < previous.metrics.receivedBytes ||
    next.transmittedBytes < previous.metrics.transmittedBytes
  ) {
    return undefined
  }

  return {
    receivedBytesPerSecond: (next.receivedBytes - previous.metrics.receivedBytes) / elapsedSeconds,
    transmittedBytesPerSecond:
      (next.transmittedBytes - previous.metrics.transmittedBytes) / elapsedSeconds,
  }
}

export function selectMostUsedDisk(disks: DiskMetrics[] | undefined): DiskMetrics | undefined {
  if (!disks?.length) return undefined
  return disks.reduce((mostUsed, disk) =>
    disk.usedPercent > mostUsed.usedPercent ? disk : mostUsed
  )
}

export function appendNetworkHistorySample(
  history: NetworkHistorySample[],
  rate: NetworkRate | undefined,
  capturedAt: number
) {
  if (!rate) return history
  return [...history, { ...rate, capturedAt }].slice(-CPU_HISTORY_SAMPLE_LIMIT)
}
