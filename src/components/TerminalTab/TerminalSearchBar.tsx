import React from "react"
import type { ISearchResultChangeEvent } from "@xterm/addon-search"
import {
  ArrowDown,
  ArrowUp,
  CaseSensitive,
  GripHorizontal,
  Regex,
  WholeWord,
  X,
} from "lucide-react"
import type { TFunction } from "i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { SearchOptionsState } from "@/components/TerminalTab/searchTypes"
import { isImeKeyEvent } from "@/lib/ime"

type TerminalSearchBarProps = {
  isSearchDragging: boolean
  onClose: () => void
  onDragStart: (event: React.PointerEvent<HTMLDivElement>) => void
  onRunSearch: (direction: "next" | "previous") => void
  onToggleOption: (option: keyof SearchOptionsState) => void
  searchInputRef: React.RefObject<HTMLInputElement | null>
  searchOptions: SearchOptionsState
  searchPosition: { x: number; y: number }
  searchQuery: string
  searchResults: ISearchResultChangeEvent
  setSearchQuery: (value: string) => void
  t: TFunction
}

export const TerminalSearchBar: React.FC<TerminalSearchBarProps> = ({
  isSearchDragging,
  onClose,
  onDragStart,
  onRunSearch,
  onToggleOption,
  searchInputRef,
  searchOptions,
  searchPosition,
  searchQuery,
  searchResults,
  setSearchQuery,
  t,
}) => {
  const handleKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    if (event.key === "Escape") {
      event.preventDefault()
      event.stopPropagation()
      onClose()
      return
    }

    if (event.key === "Enter" && event.target === searchInputRef.current) {
      event.preventDefault()
      onRunSearch(event.shiftKey ? "previous" : "next")
    }
  }

  const searchResultText = searchQuery
    ? searchResults.resultCount > 0
      ? t("terminalSearch.results", {
          current: searchResults.resultIndex >= 0 ? searchResults.resultIndex + 1 : 0,
          total: searchResults.resultCount,
          defaultValue: "{{current}} / {{total}}",
        })
      : t("terminalSearch.noResults", { defaultValue: "No results" })
    : t("terminalSearch.ready", { defaultValue: "Find in terminal" })

  return (
    <div
      className="terminal-search-bar"
      data-dragging={isSearchDragging ? "true" : undefined}
      onMouseDown={(event) => event.stopPropagation()}
      onKeyDown={handleKeyDown}
      style={{ transform: `translate(${searchPosition.x}px, ${searchPosition.y}px)` }}
    >
      <Tooltip>
        <TooltipTrigger asChild>
          <div
            className="terminal-search-drag-handle"
            onPointerDown={onDragStart}
            aria-label={t("terminalSearch.drag", { defaultValue: "Drag search box" })}
          >
            <GripHorizontal />
          </div>
        </TooltipTrigger>
        <TooltipContent>
          {t("terminalSearch.drag", { defaultValue: "Drag search box" })}
        </TooltipContent>
      </Tooltip>
      <Input
        ref={searchInputRef}
        value={searchQuery}
        onChange={(event) => setSearchQuery(event.target.value)}
        placeholder={t("terminalSearch.placeholder", { defaultValue: "Search terminal" })}
        aria-label={t("terminalSearch.placeholder", { defaultValue: "Search terminal" })}
        className="terminal-search-input"
      />
      <span
        className="terminal-search-count"
        aria-live="polite"
        data-empty={searchQuery ? undefined : "true"}
        data-missing={searchQuery && searchResults.resultCount === 0 ? "true" : undefined}
      >
        {searchResultText}
      </span>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant={searchOptions.caseSensitive ? "secondary" : "ghost"}
            size="icon-xs"
            onClick={() => onToggleOption("caseSensitive")}
            aria-pressed={searchOptions.caseSensitive}
          >
            <CaseSensitive />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("terminalSearch.caseSensitive", { defaultValue: "Match case" })}
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant={searchOptions.wholeWord ? "secondary" : "ghost"}
            size="icon-xs"
            onClick={() => onToggleOption("wholeWord")}
            aria-pressed={searchOptions.wholeWord}
          >
            <WholeWord />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("terminalSearch.wholeWord", { defaultValue: "Whole word" })}
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant={searchOptions.regex ? "secondary" : "ghost"}
            size="icon-xs"
            onClick={() => onToggleOption("regex")}
            aria-pressed={searchOptions.regex}
          >
            <Regex />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("terminalSearch.regex", { defaultValue: "Use regular expression" })}
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            type="button"
            variant="ghost"
            size="icon-xs"
            onClick={() => onRunSearch("previous")}
          >
            <ArrowUp />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("terminalSearch.previous", { defaultValue: "Previous result" })}
        </TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button type="button" variant="ghost" size="icon-xs" onClick={() => onRunSearch("next")}>
            <ArrowDown />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{t("terminalSearch.next", { defaultValue: "Next result" })}</TooltipContent>
      </Tooltip>
      <Tooltip>
        <TooltipTrigger asChild>
          <Button type="button" variant="ghost" size="icon-xs" onClick={onClose}>
            <X />
          </Button>
        </TooltipTrigger>
        <TooltipContent>
          {t("terminalSearch.close", { defaultValue: "Close search" })}
        </TooltipContent>
      </Tooltip>
    </div>
  )
}
