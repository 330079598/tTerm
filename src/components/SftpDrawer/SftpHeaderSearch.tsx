import React, { useEffect, useRef, useState } from "react"
import { FolderTree, Regex, Search, X } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Tooltip, TooltipContent, TooltipTrigger } from "@/components/ui/tooltip"
import type { SftpSearchOptions } from "@/components/SftpDrawer/sftpSearch"
import { isImeKeyEvent } from "@/lib/ime"
import { cn } from "@/lib/utils"

export interface SftpHeaderSearchProps {
  disabled: boolean
  searchError: string | null
  searchOptions: SftpSearchOptions
  searchQuery: string
  setSearchQuery: (query: string) => void
  toggleSearchOption: (option: keyof SftpSearchOptions) => void
  /** Starts a search of subfolders, when that option is on. */
  onSubmitSearch: () => void
}

function OptionButton({
  active,
  disabled,
  label,
  onClick,
  children,
}: {
  active: boolean
  disabled: boolean
  label: string
  onClick: () => void
  children: React.ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          type="button"
          variant={active ? "secondary" : "ghost"}
          size="icon-xs"
          onClick={onClick}
          aria-label={label}
          aria-pressed={active}
          disabled={disabled}
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent>{label}</TooltipContent>
    </Tooltip>
  )
}

/** The header's search button, and the search box it opens. */
export const SftpHeaderSearch: React.FC<SftpHeaderSearchProps> = ({
  disabled,
  searchError,
  searchOptions,
  searchQuery,
  setSearchQuery,
  toggleSearchOption,
  onSubmitSearch,
}) => {
  const { t } = useTranslation()
  const [isSearchOpen, setIsSearchOpen] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)

  useEffect(() => {
    if (isSearchOpen) {
      searchInputRef.current?.focus()
    }
  }, [isSearchOpen])

  const handleCloseSearch = () => {
    setIsSearchOpen(false)
    setSearchQuery("")
  }

  const handleSearchKeyDown = (event: React.KeyboardEvent<HTMLDivElement>) => {
    if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent) && searchOptions.recursive) {
      event.preventDefault()
      onSubmitSearch()
      return
    }
    if (event.key !== "Escape") return

    event.preventDefault()
    event.stopPropagation()
    handleCloseSearch()
  }

  const handleToggleSearch = () => {
    if (isSearchOpen) {
      handleCloseSearch()
      return
    }

    setIsSearchOpen(true)
  }

  const label = searchOptions.recursive
    ? t("sftp.search.recursiveLabel")
    : t("sftp.search.label", { defaultValue: "Filter current folder" })

  return (
    <>
      {isSearchOpen && (
        <div className="sftp-header-search" onKeyDown={handleSearchKeyDown}>
          <div className="sftp-search-box">
            <Search className="sftp-search-icon" />
            <Tooltip>
              <TooltipTrigger asChild>
                <Input
                  ref={searchInputRef}
                  value={searchQuery}
                  onChange={(event) => setSearchQuery(event.target.value)}
                  placeholder={
                    searchOptions.recursive
                      ? t("sftp.search.recursivePlaceholder")
                      : t("sftp.search.placeholder", { defaultValue: "Filter current folder" })
                  }
                  disabled={disabled}
                  className={cn("sftp-search-input", searchError && "border-destructive")}
                  aria-invalid={Boolean(searchError)}
                  aria-label={label}
                />
              </TooltipTrigger>
              {searchError && <TooltipContent>{searchError}</TooltipContent>}
            </Tooltip>
            <div className="sftp-search-controls">
              <OptionButton
                active={searchOptions.recursive}
                disabled={disabled}
                label={t("sftp.search.recursive")}
                onClick={() => toggleSearchOption("recursive")}
              >
                <FolderTree className="size-3" />
              </OptionButton>
              <OptionButton
                active={searchOptions.regex}
                disabled={disabled}
                label={t("sftp.search.regex", {
                  defaultValue: "Use regular expression; falls back to glob",
                })}
                onClick={() => toggleSearchOption("regex")}
              >
                <Regex className="size-3" />
              </OptionButton>
              {searchQuery && (
                <Tooltip>
                  <TooltipTrigger asChild>
                    <Button
                      variant="ghost"
                      size="icon-xs"
                      onClick={() => setSearchQuery("")}
                      aria-label={t("sftp.search.clear", { defaultValue: "Clear filter" })}
                    >
                      <X className="size-3" />
                    </Button>
                  </TooltipTrigger>
                  <TooltipContent>
                    {t("sftp.search.clear", { defaultValue: "Clear filter" })}
                  </TooltipContent>
                </Tooltip>
              )}
            </div>
          </div>
          {searchError && (
            <Tooltip>
              <TooltipTrigger asChild>
                <span className="sftp-search-error">
                  {t("sftp.search.invalidRegex", {
                    defaultValue: "Invalid regular expression",
                  })}
                </span>
              </TooltipTrigger>
              <TooltipContent>{searchError}</TooltipContent>
            </Tooltip>
          )}
        </div>
      )}
      <Tooltip>
        <TooltipTrigger asChild>
          <Button
            variant={isSearchOpen || searchQuery ? "secondary" : "ghost"}
            size="icon-sm"
            onClick={handleToggleSearch}
            disabled={disabled}
            aria-label={label}
            aria-pressed={isSearchOpen || Boolean(searchQuery)}
          >
            <Search className="size-4" />
          </Button>
        </TooltipTrigger>
        <TooltipContent>{label}</TooltipContent>
      </Tooltip>
    </>
  )
}
