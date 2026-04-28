import type { ISearchDecorationOptions } from "@xterm/addon-search"

export type SearchOptionsState = {
  caseSensitive: boolean
  wholeWord: boolean
  regex: boolean
}

export type SearchDragState = {
  pointerX: number
  pointerY: number
  positionX: number
  positionY: number
  barRect: DOMRect
  surfaceRect: DOMRect
}

export const SEARCH_DECORATIONS: ISearchDecorationOptions = {
  matchBackground: "#facc15",
  matchBorder: "#fde047",
  matchOverviewRuler: "#facc15",
  activeMatchBackground: "#fb923c",
  activeMatchBorder: "#fdba74",
  activeMatchColorOverviewRuler: "#fb923c",
}

export const DEFAULT_SEARCH_OPTIONS: SearchOptionsState = {
  caseSensitive: false,
  wholeWord: false,
  regex: false,
}
