import {
  ArrowLeft,
  Braces,
  FileCode2,
  Library,
  Pencil,
  SquarePen,
  Star,
  Tags,
  Trash2,
} from "lucide-react"
import { useTranslation } from "react-i18next"

import type { CommandLibraryData } from "@/components/CommandLibrary/useCommandLibraryData"
import type { CommandInsertion } from "@/components/CommandLibrary/useCommandInsertion"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { insertIntoComposer } from "@/lib/commandComposer"
import { formatPlaceholder } from "@/lib/commandVariables"
import { cn } from "@/lib/utils"

type CommandDetailPaneProps = Pick<
  CommandLibraryData,
  | "mobileDetailOpen"
  | "hasSelectedItem"
  | "mode"
  | "selectedRecent"
  | "setMobileDetailOpen"
  | "recentPendingId"
  | "handleRecentFavorite"
  | "selectedCommand"
  | "handleDelete"
  | "selectedVariables"
> &
  Pick<CommandInsertion, "isInserting" | "setIsInserting" | "openEditor" | "handleInsert"> & {
    canInsert: boolean
    onInsertRecent: (commandText: string) => Promise<boolean>
    onOpenChange: (open: boolean) => void
  }

/** The detail side: the selected command, what it does, and inserting it. */
export function CommandDetailPane({
  mobileDetailOpen,
  hasSelectedItem,
  mode,
  selectedRecent,
  setMobileDetailOpen,
  recentPendingId,
  handleRecentFavorite,
  selectedCommand,
  handleDelete,
  selectedVariables,
  isInserting,
  setIsInserting,
  openEditor,
  handleInsert,
  canInsert,
  onInsertRecent,
  onOpenChange,
}: CommandDetailPaneProps) {
  const { t } = useTranslation()

  return (
    <section
      className={cn(
        "bg-background/95 min-h-0 overflow-auto md:block",
        mobileDetailOpen && hasSelectedItem ? "block" : "hidden"
      )}
    >
      {mode === "recent" && selectedRecent ? (
        <div className="flex min-h-full flex-col p-5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="mb-4 w-fit md:hidden"
            onClick={() => setMobileDetailOpen(false)}
          >
            <ArrowLeft />
            {t("commandLibrary.backToList")}
          </Button>
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <h3 className="text-base font-semibold">{t("commandLibrary.recentCommand")}</h3>
              <p className="text-muted-foreground mt-2 text-sm">
                {selectedRecent.profileName ?? t("commandLibrary.scope.global")}
              </p>
            </div>
            <Button
              type="button"
              variant="ghost"
              size="icon-sm"
              disabled={recentPendingId === selectedRecent.id}
              aria-label={t("commandLibrary.favorite")}
              onClick={() => void handleRecentFavorite(selectedRecent)}
            >
              {recentPendingId === selectedRecent.id ? <Spinner /> : <Star />}
            </Button>
          </div>
          <div className="border-border/80 bg-muted/30 mt-5 min-h-32 rounded-md border p-4 shadow-inner">
            <div className="text-muted-foreground mb-3 flex items-center gap-2 text-xs font-medium">
              <Braces className="size-3.5" />
              {t("commandLibrary.commandPreview")}
            </div>
            <pre className="text-foreground overflow-x-auto font-mono text-sm leading-6 break-all whitespace-pre-wrap">
              {selectedRecent.commandText}
            </pre>
          </div>
          <dl className="mt-5 grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
            <div>
              <dt className="text-muted-foreground text-xs">{t("commandLibrary.usedLabel")}</dt>
              <dd className="mt-1 font-medium tabular-nums">
                {t("commandLibrary.recentUseCount", { count: selectedRecent.useCount })}
              </dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-xs">{t("commandLibrary.lastUsedLabel")}</dt>
              <dd className="mt-1 font-medium">
                {new Intl.DateTimeFormat(undefined, {
                  dateStyle: "short",
                  timeStyle: "short",
                }).format(selectedRecent.lastUsedAt)}
              </dd>
            </div>
          </dl>
          <div className="mt-auto flex flex-col gap-2 pt-6 sm:flex-row">
            <Button
              type="button"
              className="w-full sm:w-auto"
              disabled={!canInsert || isInserting}
              onClick={async () => {
                setIsInserting(true)
                try {
                  if (await onInsertRecent(selectedRecent.commandText)) onOpenChange(false)
                } finally {
                  setIsInserting(false)
                }
              }}
            >
              {isInserting ? <Spinner /> : <FileCode2 />}
              {isInserting ? t("commandLibrary.inserting") : t("commandLibrary.insert")}
            </Button>
            <Button
              type="button"
              variant="outline"
              className="w-full sm:w-auto"
              onClick={() => {
                insertIntoComposer(selectedRecent.commandText)
                onOpenChange(false)
              }}
            >
              <SquarePen />
              {t("commandLibrary.insertIntoComposer")}
            </Button>
          </div>
        </div>
      ) : selectedCommand ? (
        <div className="flex min-h-full flex-col p-5">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="mb-4 w-fit md:hidden"
            onClick={() => setMobileDetailOpen(false)}
          >
            <ArrowLeft />
            {t("commandLibrary.backToList")}
          </Button>
          <div className="flex items-start justify-between gap-4">
            <div className="min-w-0">
              <div className="flex flex-wrap items-center gap-2">
                <h3 className="text-base font-semibold break-words">{selectedCommand.name}</h3>
                {selectedCommand.isFavorite && (
                  <Star className="size-4 fill-current text-amber-500" aria-hidden="true" />
                )}
              </div>
              {selectedCommand.description && (
                <p className="text-muted-foreground mt-2 text-sm leading-6 break-words">
                  {selectedCommand.description}
                </p>
              )}
            </div>
            <div className="flex shrink-0 items-center gap-1">
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={t("commandLibrary.edit")}
                onClick={() => {
                  openEditor(selectedCommand)
                }}
              >
                <Pencil />
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                className="hover:text-destructive"
                aria-label={t("commandLibrary.deleteAction")}
                onClick={() => void handleDelete(selectedCommand)}
              >
                <Trash2 />
              </Button>
            </div>
          </div>

          <div className="border-border/80 bg-muted/30 mt-5 min-h-32 rounded-md border p-4 shadow-inner">
            <div className="text-muted-foreground mb-3 flex items-center gap-2 text-xs font-medium">
              <Braces className="size-3.5" />
              {t("commandLibrary.commandPreview")}
            </div>
            <pre className="text-foreground overflow-x-auto font-mono text-sm leading-6 break-all whitespace-pre-wrap">
              {selectedCommand.commandText}
            </pre>
          </div>

          <dl className="mt-5 grid grid-cols-2 gap-x-6 gap-y-4 text-sm">
            <div>
              <dt className="text-muted-foreground text-xs">{t("commandLibrary.scopeLabel")}</dt>
              <dd className="mt-1 font-medium">
                {selectedCommand.scopeType === "global"
                  ? t("commandLibrary.scope.global")
                  : t("commandLibrary.scope.profileBadge")}
              </dd>
            </div>
            <div>
              <dt className="text-muted-foreground text-xs">{t("commandLibrary.usedLabel")}</dt>
              <dd className="mt-1 font-medium tabular-nums">
                {t("commandLibrary.usedCount", { count: selectedCommand.useCount })}
              </dd>
            </div>
            {selectedVariables.length > 0 && (
              <div className="col-span-2">
                <dt className="text-muted-foreground text-xs">
                  {t("commandLibrary.variables.title")}
                </dt>
                <dd className="mt-1.5 flex flex-wrap gap-2">
                  {selectedVariables.map((variable) => (
                    <Badge
                      key={variable.name}
                      variant="outline"
                      className="rounded-md font-mono"
                      title={`${variable.label} · ${t(`commandLibrary.variables.types.${variable.valueType}`)}`}
                    >
                      {formatPlaceholder(variable.name)}
                    </Badge>
                  ))}
                </dd>
              </div>
            )}
            {selectedCommand.confirmBeforeRun && (
              <div className="col-span-2">
                <dt className="text-muted-foreground text-xs">
                  {t("commandLibrary.form.confirmBeforeRun")}
                </dt>
                <dd className="mt-1 font-medium">{t("commandLibrary.confirmEnabled")}</dd>
              </div>
            )}
          </dl>

          <section
            className="border-border/70 bg-muted/15 mt-5 rounded-md border p-3"
            aria-label={t("commandLibrary.tagsLabel")}
          >
            <div className="flex items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-2">
                <Tags className="text-muted-foreground size-3.5 shrink-0" aria-hidden="true" />
                <h4 className="text-muted-foreground text-xs font-medium">
                  {t("commandLibrary.tagsLabel")}
                </h4>
              </div>
              <Button
                type="button"
                variant="ghost"
                size="sm"
                className="h-8 shrink-0 gap-1.5 px-2 text-xs"
                aria-label={t("commandLibrary.editTags")}
                onClick={() => openEditor(selectedCommand)}
              >
                <Pencil className="size-3.5" />
                {t("commandLibrary.editTags")}
              </Button>
            </div>
            {selectedCommand.tags.length > 0 ? (
              <div className="mt-2 flex flex-wrap gap-2">
                {selectedCommand.tags.map((tag) => (
                  <Badge key={tag.toLocaleLowerCase()} variant="secondary" className="rounded-md">
                    {tag}
                  </Badge>
                ))}
              </div>
            ) : (
              <p className="text-muted-foreground mt-2 text-xs">{t("commandLibrary.noTags")}</p>
            )}
          </section>

          <div className="mt-auto pt-6">
            <div className="flex flex-col gap-2 sm:flex-row">
              <Button
                type="button"
                className="w-full sm:w-auto"
                disabled={!canInsert || isInserting}
                onClick={() => void handleInsert(selectedCommand)}
              >
                {isInserting ? <Spinner /> : <FileCode2 />}
                {isInserting ? t("commandLibrary.inserting") : t("commandLibrary.insert")}
              </Button>
              <Button
                type="button"
                variant="outline"
                className="w-full sm:w-auto"
                disabled={isInserting}
                onClick={() => void handleInsert(selectedCommand, "composer")}
              >
                <SquarePen />
                {t("commandLibrary.insertIntoComposer")}
              </Button>
            </div>
            {!canInsert && (
              <p className="text-muted-foreground mt-2 text-xs">
                {t("commandLibrary.selectTerminalHint")}
              </p>
            )}
          </div>
        </div>
      ) : (
        <div className="text-muted-foreground flex min-h-full flex-col items-center justify-center px-8 text-center">
          <Library className="size-7" />
          <p className="mt-3 text-sm">{t("commandLibrary.selectHint")}</p>
        </div>
      )}
    </section>
  )
}
