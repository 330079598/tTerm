import { AlertCircle, Clock3, FileCode2, Plus, Search, Star } from "lucide-react"
import { useTranslation } from "react-i18next"

import type { CommandLibraryData } from "@/components/CommandLibrary/useCommandLibraryData"
import type { CommandInsertion } from "@/components/CommandLibrary/useCommandInsertion"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { ScrollArea } from "@/components/ui/scroll-area"
import { Spinner } from "@/components/ui/spinner"
import { cn } from "@/lib/utils"

type CommandListPaneProps = Pick<
  CommandLibraryData,
  | "mobileDetailOpen"
  | "hasSelectedItem"
  | "searchInputRef"
  | "searchQuery"
  | "setSearchQuery"
  | "mode"
  | "setMode"
  | "isLoading"
  | "loadError"
  | "loadCommands"
  | "filteredRecentCommands"
  | "filteredCommands"
  | "isFiltering"
  | "selectedId"
  | "setSelectedId"
  | "setMobileDetailOpen"
  | "favoritePendingId"
  | "handleFavorite"
  | "commands"
  | "selectedRecentId"
  | "setSelectedRecentId"
  | "recentPendingId"
  | "handleRecentFavorite"
> &
  Pick<CommandInsertion, "openEditor">

/** The list side: search, the filter, and the saved or recent commands. */
export function CommandListPane({
  mobileDetailOpen,
  hasSelectedItem,
  searchInputRef,
  searchQuery,
  setSearchQuery,
  mode,
  setMode,
  isLoading,
  loadError,
  loadCommands,
  filteredRecentCommands,
  filteredCommands,
  isFiltering,
  selectedId,
  setSelectedId,
  setMobileDetailOpen,
  favoritePendingId,
  handleFavorite,
  commands,
  selectedRecentId,
  setSelectedRecentId,
  recentPendingId,
  handleRecentFavorite,
  openEditor,
}: CommandListPaneProps) {
  const { t } = useTranslation()

  return (
    <section
      className={cn(
        "border-border/80 bg-muted/10 min-h-0 flex-col border-b md:flex md:border-r md:border-b-0",
        mobileDetailOpen && hasSelectedItem ? "hidden" : "flex"
      )}
    >
      <div className="border-border/70 bg-background/70 space-y-3 border-b p-4">
        <div className="relative">
          <Search className="text-muted-foreground pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2" />
          <Input
            ref={searchInputRef}
            type="search"
            value={searchQuery}
            placeholder={t("commandLibrary.searchPlaceholder")}
            aria-label={t("commandLibrary.searchPlaceholder")}
            className="pl-9"
            onChange={(event) => setSearchQuery(event.target.value)}
          />
        </div>
        <div
          className="bg-muted/55 grid grid-cols-3 rounded-md p-1"
          role="group"
          aria-label={t("commandLibrary.filterLabel")}
        >
          {(["all", "favorites", "recent"] as const).map((value) => (
            <button
              key={value}
              type="button"
              aria-pressed={mode === value}
              onClick={() => setMode(value)}
              className={cn(
                "focus-visible:ring-ring/50 min-h-9 rounded-sm px-3 text-xs font-medium transition-colors outline-none focus-visible:ring-[3px]",
                mode === value
                  ? "bg-background text-foreground shadow-xs"
                  : "text-muted-foreground hover:text-foreground"
              )}
            >
              {t(`commandLibrary.filters.${value}`)}
            </button>
          ))}
        </div>
      </div>

      <ScrollArea className="min-h-0 flex-1">
        {isLoading && (
          <div className="flex min-h-48 items-center justify-center" role="status">
            <Spinner size={20} className="text-muted-foreground" />
            <span className="sr-only">{t("commandLibrary.loading")}</span>
          </div>
        )}

        {!isLoading && loadError && (
          <div className="flex min-h-48 flex-col items-center justify-center px-6 text-center">
            <AlertCircle className="text-destructive size-5" />
            <p className="mt-3 text-sm font-medium">{t("commandLibrary.loadFailed")}</p>
            <p className="text-muted-foreground mt-1 max-w-xs text-xs break-words">{loadError}</p>
            <Button
              type="button"
              variant="outline"
              size="sm"
              className="mt-4"
              onClick={loadCommands}
            >
              {t("commandLibrary.retry")}
            </Button>
          </div>
        )}

        {!isLoading &&
          !loadError &&
          (mode === "recent"
            ? filteredRecentCommands.length === 0
            : filteredCommands.length === 0) && (
            <div className="flex min-h-52 flex-col items-center justify-center px-6 text-center">
              {mode === "recent" ? (
                <Clock3 className="text-muted-foreground size-6" />
              ) : (
                <FileCode2 className="text-muted-foreground size-6" />
              )}
              <p className="mt-3 text-sm font-medium">
                {mode === "recent"
                  ? t("commandLibrary.recentEmpty")
                  : isFiltering
                    ? t("commandLibrary.noResults")
                    : t("commandLibrary.empty")}
              </p>
              <p className="text-muted-foreground mt-1 max-w-xs text-xs leading-5">
                {mode === "recent"
                  ? t("commandLibrary.recentEmptyHint")
                  : isFiltering
                    ? t("commandLibrary.noResultsHint")
                    : t("commandLibrary.emptyHint")}
              </p>
              {!isFiltering && mode !== "recent" && (
                <Button
                  type="button"
                  variant="outline"
                  size="sm"
                  className="mt-4"
                  onClick={() => {
                    openEditor(null)
                  }}
                >
                  <Plus />
                  {t("commandLibrary.newCommand")}
                </Button>
              )}
            </div>
          )}

        {!isLoading && !loadError && mode !== "recent" && filteredCommands.length > 0 && (
          <div role="listbox" aria-label={t("commandLibrary.listLabel")} className="py-1">
            {filteredCommands.map((command) => (
              <div
                key={command.id}
                className={cn(
                  "group border-border/60 hover:bg-muted/35 relative border-b last:border-b-0",
                  selectedId === command.id && "bg-primary/8 hover:bg-primary/10"
                )}
              >
                {selectedId === command.id && (
                  <span className="bg-primary absolute inset-y-2 left-0 w-0.5 rounded-r" />
                )}
                <button
                  type="button"
                  role="option"
                  aria-selected={selectedId === command.id}
                  className="focus-visible:ring-ring/50 w-full px-4 py-3.5 pr-12 text-left outline-none focus-visible:ring-[3px] focus-visible:ring-inset"
                  onClick={() => {
                    setSelectedId(command.id)
                    setMobileDetailOpen(true)
                  }}
                >
                  <span className="block truncate text-sm font-medium">{command.name}</span>
                  <code className="text-muted-foreground mt-1.5 block truncate font-mono text-xs">
                    {command.commandText}
                  </code>
                  <span className="mt-2 flex min-h-5 flex-wrap items-center gap-1.5">
                    {command.scopeType === "profile" && (
                      <Badge variant="outline" className="rounded-md px-1.5 py-0 text-[10px]">
                        {t("commandLibrary.scope.profileBadge")}
                      </Badge>
                    )}
                    {command.tags.slice(0, 2).map((tag) => (
                      <Badge
                        key={tag.toLocaleLowerCase()}
                        variant="secondary"
                        className="max-w-24 rounded-md px-1.5 py-0 text-[10px]"
                      >
                        <span className="truncate">{tag}</span>
                      </Badge>
                    ))}
                  </span>
                </button>
                <button
                  type="button"
                  disabled={favoritePendingId === command.id}
                  aria-label={
                    command.isFavorite
                      ? t("commandLibrary.unfavorite")
                      : t("commandLibrary.favorite")
                  }
                  className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 absolute top-3 right-2 flex size-9 items-center justify-center rounded-md transition-colors outline-none focus-visible:ring-[3px] disabled:opacity-50"
                  onClick={() => void handleFavorite(command)}
                >
                  {favoritePendingId === command.id ? (
                    <Spinner size={14} />
                  ) : (
                    <Star
                      className={cn(
                        "size-3.5",
                        command.isFavorite && "fill-current text-amber-500"
                      )}
                    />
                  )}
                </button>
              </div>
            ))}
          </div>
        )}

        {!isLoading && !loadError && mode === "recent" && filteredRecentCommands.length > 0 && (
          <div role="listbox" aria-label={t("commandLibrary.recentListLabel")} className="py-1">
            {filteredRecentCommands.map((recent) => {
              const saved = commands.find(
                (command) =>
                  command.commandText.trim() === recent.commandText.trim() &&
                  command.scopeId === recent.profileId
              )
              return (
                <div
                  key={recent.id}
                  className={cn(
                    "group border-border/60 hover:bg-muted/35 relative border-b last:border-b-0",
                    selectedRecentId === recent.id && "bg-primary/8 hover:bg-primary/10"
                  )}
                >
                  {selectedRecentId === recent.id && (
                    <span className="bg-primary absolute inset-y-2 left-0 w-0.5 rounded-r" />
                  )}
                  <button
                    type="button"
                    role="option"
                    aria-selected={selectedRecentId === recent.id}
                    className="focus-visible:ring-ring/50 w-full px-4 py-3.5 pr-12 text-left outline-none focus-visible:ring-[3px] focus-visible:ring-inset"
                    onClick={() => {
                      setSelectedRecentId(recent.id)
                      setMobileDetailOpen(true)
                    }}
                  >
                    <code className="block truncate font-mono text-sm font-medium">
                      {recent.commandText}
                    </code>
                    <span className="text-muted-foreground mt-2 flex items-center gap-2 text-xs">
                      <span>{recent.profileName ?? t("commandLibrary.scope.global")}</span>
                      <span aria-hidden="true">·</span>
                      <span>{t("commandLibrary.recentUseCount", { count: recent.useCount })}</span>
                    </span>
                  </button>
                  <button
                    type="button"
                    disabled={recentPendingId === recent.id}
                    aria-label={
                      saved?.isFavorite
                        ? t("commandLibrary.unfavorite")
                        : t("commandLibrary.favorite")
                    }
                    className="text-muted-foreground hover:bg-accent hover:text-foreground focus-visible:ring-ring/50 absolute top-3 right-2 flex size-9 items-center justify-center rounded-md transition-colors outline-none focus-visible:ring-[3px] disabled:opacity-50"
                    onClick={() => void handleRecentFavorite(recent)}
                  >
                    {recentPendingId === recent.id ? (
                      <Spinner size={14} />
                    ) : (
                      <Star
                        className={cn(
                          "size-3.5",
                          saved?.isFavorite && "fill-current text-amber-500"
                        )}
                      />
                    )}
                  </button>
                </div>
              )
            })}
          </div>
        )}
      </ScrollArea>
    </section>
  )
}
