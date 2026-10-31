import * as React from "react"
import { Check, ChevronDown, Search } from "lucide-react"

import { Input } from "@/components/ui/input"
import { isImeKeyEvent } from "@/lib/ime"
import {
  filterOptionGroups,
  type SearchableOption,
  type SearchableOptionGroup,
} from "@/lib/optionSearch"
import { cn } from "@/lib/utils"

export interface SearchableSelectOption extends SearchableOption {
  /** Shown before the label, in the list and on the button once chosen. */
  icon?: React.ReactNode
}

export type SearchableSelectGroup = SearchableOptionGroup<SearchableSelectOption>

interface SearchableSelectProps {
  id: string
  value: string
  groups: SearchableSelectGroup[]
  onChange: (value: string) => void
  /** What the button shows for the current value. */
  triggerLabel: string
  triggerIcon?: React.ReactNode
  searchLabel: string
  listLabel: string
  /** Shown when nothing matches the search; omit it while options are still loading. */
  emptyText?: string
  /** Shown below the options, such as a note that more are loading. */
  footer?: React.ReactNode
}

/**
 * A button that opens a searchable list in place, for a choice among more
 * options than a plain select lets anyone scan. An option may be listed in
 * more than one group.
 */
export function SearchableSelect({
  id,
  value,
  groups,
  onChange,
  triggerLabel,
  triggerIcon,
  searchLabel,
  listLabel,
  emptyText,
  footer,
}: SearchableSelectProps) {
  const [open, setOpen] = React.useState(false)
  const [query, setQuery] = React.useState("")
  const [highlightedKey, setHighlightedKey] = React.useState<string | null>(null)
  const containerRef = React.useRef<HTMLDivElement>(null)
  const triggerRef = React.useRef<HTMLButtonElement>(null)
  const listRef = React.useRef<HTMLDivElement>(null)

  const visibleGroups = filterOptionGroups(groups, query)
  const entries = visibleGroups.flatMap((group) =>
    group.options.map((option) => ({ ...option, key: `${group.label ?? ""}:${option.value}` }))
  )
  const highlighted = entries.find((entry) => entry.key === highlightedKey) ?? entries[0]

  const close = (refocus: boolean) => {
    setOpen(false)
    setQuery("")
    setHighlightedKey(null)
    if (refocus) triggerRef.current?.focus()
  }

  const choose = (next: string) => {
    onChange(next)
    close(true)
  }

  // Clicking anywhere else closes the list.
  React.useEffect(() => {
    if (!open) return
    const handlePointerDown = (event: PointerEvent) => {
      if (containerRef.current?.contains(event.target as Node)) return
      setOpen(false)
      setQuery("")
      setHighlightedKey(null)
    }
    document.addEventListener("pointerdown", handlePointerDown)
    return () => document.removeEventListener("pointerdown", handlePointerDown)
  }, [open])

  const highlightedEntryKey = highlighted?.key
  React.useEffect(() => {
    if (!open || !highlightedEntryKey) return
    listRef.current
      ?.querySelector<HTMLElement>('[aria-selected="true"]')
      ?.scrollIntoView({ block: "nearest" })
  }, [highlightedEntryKey, open])

  const handleKeyDown = (event: React.KeyboardEvent) => {
    if (isImeKeyEvent(event.nativeEvent)) return
    if (event.key === "Escape") {
      // Closes the list, not the dialog around it.
      event.preventDefault()
      event.stopPropagation()
      close(true)
      return
    }
    if (event.key === "Enter") {
      // The field sits in a form; Enter must not submit it.
      event.preventDefault()
      if (highlighted) choose(highlighted.value)
      return
    }
    if (entries.length === 0) return
    const index = highlighted ? entries.indexOf(highlighted) : -1
    const last = entries.length - 1
    const target = {
      ArrowDown: Math.min(last, index + 1),
      ArrowUp: Math.max(0, index - 1),
      PageDown: Math.min(last, index + 10),
      PageUp: Math.max(0, index - 10),
    }[event.key]
    if (target !== undefined) {
      event.preventDefault()
      setHighlightedKey(entries[target].key)
    }
  }

  const openList = () => {
    // Start on the current choice so the arrow keys continue from it.
    const current = entries.find((entry) => entry.value === value)
    setHighlightedKey(current?.key ?? null)
    setOpen(true)
  }

  return (
    <div ref={containerRef}>
      <button
        ref={triggerRef}
        id={id}
        type="button"
        aria-haspopup="listbox"
        aria-expanded={open}
        onClick={() => (open ? close(false) : openList())}
        className="border-input bg-background focus-visible:border-ring flex h-9 w-full items-center gap-2 rounded-md border px-3 text-left text-sm shadow-xs outline-none"
      >
        {triggerIcon}
        <span className="min-w-0 flex-1 truncate">{triggerLabel}</span>
        <ChevronDown className="text-muted-foreground size-4 shrink-0" />
      </button>

      {open && (
        <div className="mt-1 rounded-md border shadow-sm" onKeyDown={handleKeyDown}>
          <div className="relative border-b p-1">
            <Search
              size={14}
              className="text-muted-foreground pointer-events-none absolute top-1/2 left-3.5 -translate-y-1/2"
            />
            <Input
              autoFocus
              value={query}
              onChange={(event) => {
                setQuery(event.target.value)
                setHighlightedKey(null)
              }}
              placeholder={searchLabel}
              aria-label={searchLabel}
              aria-controls={`${id}-list`}
              aria-activedescendant={highlighted ? `${id}-${highlighted.key}` : undefined}
              className="h-8 border-0 pl-8 shadow-none focus-visible:ring-0"
            />
          </div>
          <div
            ref={listRef}
            id={`${id}-list`}
            role="listbox"
            aria-label={listLabel}
            className="max-h-64 overflow-y-auto p-1"
          >
            {visibleGroups.map((group) => (
              <div key={group.label ?? ""} role="group" aria-label={group.label ?? undefined}>
                {group.label && (
                  <div className="text-muted-foreground px-2 pt-2 pb-1 text-xs font-medium">
                    {group.label}
                  </div>
                )}
                {group.options.map((option) => {
                  const key = `${group.label ?? ""}:${option.value}`
                  return (
                    <div
                      key={key}
                      id={`${id}-${key}`}
                      role="option"
                      aria-selected={key === highlighted?.key}
                      onPointerMove={() => setHighlightedKey(key)}
                      onClick={() => choose(option.value)}
                      className={cn(
                        "flex cursor-default items-center gap-2 rounded-sm px-2 py-1.5 text-sm",
                        key === highlighted?.key && "bg-muted"
                      )}
                    >
                      {option.icon}
                      <span className="min-w-0 flex-1 truncate">
                        {option.label}
                        {option.detail && (
                          <span className="text-muted-foreground ml-2 text-xs">
                            {option.detail}
                          </span>
                        )}
                      </span>
                      {option.value === value && <Check className="size-3.5 shrink-0" />}
                    </div>
                  )
                })}
              </div>
            ))}
            {entries.length === 0 && emptyText && (
              <p className="text-muted-foreground px-2 py-4 text-center text-sm">{emptyText}</p>
            )}
            {footer}
          </div>
        </div>
      )}
    </div>
  )
}
