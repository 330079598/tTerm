import React, { useMemo } from "react"

import { type CpuHistorySample, normalizePercent } from "@/components/TerminalTab/serverMetrics"

const CPU_SPARKLINE_WIDTH = 78
const CPU_SPARKLINE_HEIGHT = 16
const CPU_SPARKLINE_MAX_CORE_LINES = 6

export function PercentValue({ value }: { value?: number }) {
  const percent =
    value === undefined || Number.isNaN(value)
      ? "--"
      : String(Math.max(0, Math.min(100, Math.round(value))))

  return (
    <span className="server-monitor-percent-value">
      <span className="server-monitor-percent-number">{percent}</span>
      {percent !== "--" && <span>%</span>}
    </span>
  )
}

function buildCpuSparklinePoints(values: number[]) {
  const paddingX = 2
  const paddingY = 2
  const plotWidth = CPU_SPARKLINE_WIDTH - paddingX * 2
  const plotHeight = CPU_SPARKLINE_HEIGHT - paddingY * 2

  if (values.length === 0) {
    return []
  }

  return values.map((value, index) => {
    const x =
      values.length === 1
        ? CPU_SPARKLINE_WIDTH / 2
        : paddingX + (index / (values.length - 1)) * plotWidth
    const y = paddingY + (1 - normalizePercent(value) / 100) * plotHeight
    return { x, y }
  })
}

function buildSmoothPath(points: Array<{ x: number; y: number }>) {
  if (points.length < 2) {
    return ""
  }

  return points.reduce((path, point, index) => {
    if (index === 0) {
      return `M${point.x},${point.y}`
    }

    const previous = points[index - 1]
    const controlX = previous.x + (point.x - previous.x) * 0.5
    return `${path} C${controlX},${previous.y} ${controlX},${point.y} ${point.x},${point.y}`
  }, "")
}

function getCoreSeries(samples: CpuHistorySample[]) {
  const valuesByCore = new Map<string, number[]>()
  samples.forEach((sample, sampleIndex) => {
    for (const core of sample.cores) {
      let values = valuesByCore.get(core.id)
      if (!values) {
        values = new Array<number>(samples.length).fill(0)
        valuesByCore.set(core.id, values)
      }
      values[sampleIndex] = core.percent
    }
  })

  return Array.from(valuesByCore, ([id, values]) => ({
    id,
    values,
    latest: values[values.length - 1] ?? 0,
  }))
    .sort((first, second) => second.latest - first.latest)
    .slice(0, CPU_SPARKLINE_MAX_CORE_LINES)
}

export const CpuSparkline = React.memo(function CpuSparkline({
  samples,
}: {
  samples: CpuHistorySample[]
}) {
  const { hasLine, linePath, areaPath, coreSeries, latestPoint } = useMemo(() => {
    const values = samples.map((sample) => sample.total)
    const points = buildCpuSparklinePoints(values)
    const hasLine = points.length > 1
    const linePath = hasLine ? buildSmoothPath(points) : ""
    const areaPath = hasLine
      ? `${linePath} L${points[points.length - 1].x},${CPU_SPARKLINE_HEIGHT - 1} L${
          points[0].x
        },${CPU_SPARKLINE_HEIGHT - 1} Z`
      : ""
    const coreSeries = getCoreSeries(samples).map((series) => ({
      id: series.id,
      path: buildSmoothPath(buildCpuSparklinePoints(series.values)),
    }))
    const latestPoint = points[points.length - 1]
    return { hasLine, linePath, areaPath, coreSeries, latestPoint }
  }, [samples])

  return (
    <span className="server-monitor-cpu-chart" aria-hidden="true">
      <svg
        viewBox={`0 0 ${CPU_SPARKLINE_WIDTH} ${CPU_SPARKLINE_HEIGHT}`}
        preserveAspectRatio="none"
        focusable="false"
      >
        <path
          className="server-monitor-cpu-threshold"
          d={`M2,${2 + (1 - 0.7) * (CPU_SPARKLINE_HEIGHT - 4)} L${
            CPU_SPARKLINE_WIDTH - 2
          },${2 + (1 - 0.7) * (CPU_SPARKLINE_HEIGHT - 4)}`}
        />
        {hasLine &&
          coreSeries.map((series, index) =>
            series.path ? (
              <path
                key={series.id}
                className="server-monitor-cpu-core-line"
                data-core-index={index}
                d={series.path}
              />
            ) : null
          )}
        {hasLine && <path className="server-monitor-cpu-area" d={areaPath} />}
        {hasLine && <path className="server-monitor-cpu-line" d={linePath} />}
        {!hasLine && <path className="server-monitor-cpu-placeholder" d="M4,10 L64,10" />}
        {latestPoint && (
          <circle
            className="server-monitor-cpu-dot"
            cx={latestPoint.x}
            cy={latestPoint.y}
            r="1.8"
          />
        )}
      </svg>
    </span>
  )
})

export const MetricItem = React.memo(function MetricItem({
  ariaLabel,
  className,
  icon,
  label,
  labelTitle,
  onClick,
  value,
}: {
  ariaLabel?: string
  className?: string
  icon: React.ReactNode
  label: string
  labelTitle?: string
  onClick?: () => void
  value: React.ReactNode
}) {
  if (onClick) {
    return (
      <button
        type="button"
        className={`server-monitor-metric is-clickable ${className ?? ""}`.trim()}
        onClick={onClick}
        aria-label={ariaLabel}
      >
        {icon}
        <span className="server-monitor-metric-label" title={labelTitle}>
          {label}
        </span>
        <span className="server-monitor-metric-value">{value}</span>
      </button>
    )
  }

  return (
    <span className={`server-monitor-metric ${className ?? ""}`.trim()}>
      {icon}
      <span className="server-monitor-metric-label" title={labelTitle}>
        {label}
      </span>
      <span className="server-monitor-metric-value">{value}</span>
    </span>
  )
})
