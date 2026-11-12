import React, { useEffect, useMemo, useRef, useState } from "react"

import { SFTP_COLUMN_WIDTH_STORAGE_KEY } from "@/components/SftpDrawer/sftpView"
import {
  cachedFrontendState,
  isFrontendStateHydrated,
  writeFrontendStateValue,
} from "@/lib/frontendStateCache"

export const SFTP_COLUMN_DEFAULT_WEIGHTS = [300, 170, 100, 100, 126, 148] as const
const SFTP_COLUMN_MIN_WEIGHTS = [180, 130, 78, 70, 92, 110] as const

type ResizeState = {
  index: number
  startX: number
  pxPerWeight: number
  startWeights: number[]
}

function normalizeColumnWeight(weight: number) {
  return Math.round(weight * 10) / 10
}

function resizeColumnPair(startWeights: number[], index: number, deltaWeight: number) {
  const nextWeights = [...startWeights]
  const leftMin = SFTP_COLUMN_MIN_WEIGHTS[index]
  const rightMin = SFTP_COLUMN_MIN_WEIGHTS[index + 1]
  const pairTotal = startWeights[index] + startWeights[index + 1]
  const minLeft = Math.min(leftMin, pairTotal - rightMin)
  const maxLeft = Math.max(minLeft, pairTotal - rightMin)
  const requestedLeft = startWeights[index] + deltaWeight
  const nextLeft = Math.min(Math.max(requestedLeft, minLeft), maxLeft)

  nextWeights[index] = normalizeColumnWeight(nextLeft)
  nextWeights[index + 1] = normalizeColumnWeight(pairTotal - nextLeft)
  return nextWeights
}

/** Reads the saved widths, using the loaded cache or the 0.8.x localStorage key. */
export function loadSftpColumnWeights(
  storage: Pick<Storage, "getItem"> = window.localStorage
): number[] {
  try {
    const storedWidths = isFrontendStateHydrated()
      ? cachedFrontendState("sftpColumnWidths")
      : storage.getItem(SFTP_COLUMN_WIDTH_STORAGE_KEY)
    if (storedWidths === undefined || storedWidths === null) {
      return [...SFTP_COLUMN_DEFAULT_WEIGHTS]
    }

    const parsedWidths =
      typeof storedWidths === "string" ? (JSON.parse(storedWidths) as unknown) : storedWidths
    if (
      !Array.isArray(parsedWidths) ||
      parsedWidths.length !== SFTP_COLUMN_DEFAULT_WEIGHTS.length
    ) {
      return [...SFTP_COLUMN_DEFAULT_WEIGHTS]
    }

    return SFTP_COLUMN_DEFAULT_WEIGHTS.map((defaultWidth, index) => {
      const width = parsedWidths[index]
      return typeof width === "number" && Number.isFinite(width)
        ? Math.max(SFTP_COLUMN_MIN_WEIGHTS[index], normalizeColumnWeight(width))
        : defaultWidth
    })
  } catch {
    return [...SFTP_COLUMN_DEFAULT_WEIGHTS]
  }
}

/**
 * The listing's column widths: resized by dragging or the keyboard, and
 * kept across sessions.
 */
export function useSftpColumnWidths() {
  const [columnWeights, setColumnWeights] = useState(loadSftpColumnWeights)
  const resizeStateRef = useRef<ResizeState | null>(null)
  const tableShellRef = useRef<HTMLDivElement>(null)
  const gridTemplateColumns = useMemo(
    () => columnWeights.map((width) => `minmax(0, ${width}fr)`).join(" "),
    [columnWeights]
  )
  const compactGridTemplateColumns = useMemo(
    () =>
      columnWeights
        .slice(0, 3)
        .map((width) => `minmax(0, ${width}fr)`)
        .join(" "),
    [columnWeights]
  )
  const minimalGridTemplateColumns = useMemo(
    () =>
      columnWeights
        .slice(0, 2)
        .map((width) => `minmax(0, ${width}fr)`)
        .join(" "),
    [columnWeights]
  )
  const singleGridTemplateColumns = useMemo(
    () => `minmax(0, ${columnWeights[0]}fr)`,
    [columnWeights]
  )
  const tableGridStyle = useMemo(
    () =>
      ({
        "--sftp-grid-template": gridTemplateColumns,
        "--sftp-grid-template-compact": compactGridTemplateColumns,
        "--sftp-grid-template-minimal": minimalGridTemplateColumns,
        "--sftp-grid-template-single": singleGridTemplateColumns,
      }) as React.CSSProperties,
    [
      compactGridTemplateColumns,
      gridTemplateColumns,
      minimalGridTemplateColumns,
      singleGridTemplateColumns,
    ]
  )

  const startColumnResize = React.useCallback(
    (index: number, event: React.MouseEvent<HTMLSpanElement>) => {
      event.preventDefault()
      event.stopPropagation()
      const totalWeight = columnWeights.reduce((sum, width) => sum + width, 0)
      const availableWidth = tableShellRef.current?.clientWidth ?? totalWeight
      resizeStateRef.current = {
        index,
        startX: event.clientX,
        pxPerWeight: Math.max(availableWidth / totalWeight, 0.1),
        startWeights: columnWeights,
      }
      document.body.classList.add("sftp-column-resizing")
    },
    [columnWeights]
  )

  const adjustColumnWidth = React.useCallback((index: number, delta: number) => {
    setColumnWeights((currentWeights) => resizeColumnPair(currentWeights, index, delta))
  }, [])

  useEffect(() => {
    if (isFrontendStateHydrated()) {
      writeFrontendStateValue("sftpColumnWidths", columnWeights)
      return
    }
    window.localStorage.setItem(SFTP_COLUMN_WIDTH_STORAGE_KEY, JSON.stringify(columnWeights))
  }, [columnWeights])

  useEffect(() => {
    const handleMouseMove = (event: MouseEvent) => {
      const resizeState = resizeStateRef.current
      if (!resizeState) {
        return
      }

      const delta = event.clientX - resizeState.startX
      setColumnWeights(
        resizeColumnPair(
          resizeState.startWeights,
          resizeState.index,
          delta / resizeState.pxPerWeight
        )
      )
    }

    const stopResize = () => {
      resizeStateRef.current = null
      document.body.classList.remove("sftp-column-resizing")
    }

    window.addEventListener("mousemove", handleMouseMove)
    window.addEventListener("mouseup", stopResize)
    return () => {
      document.body.classList.remove("sftp-column-resizing")
      window.removeEventListener("mousemove", handleMouseMove)
      window.removeEventListener("mouseup", stopResize)
    }
  }, [])

  return { columnWeights, tableShellRef, tableGridStyle, startColumnResize, adjustColumnWidth }
}
