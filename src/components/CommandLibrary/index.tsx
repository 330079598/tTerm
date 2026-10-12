import { Library, Plus, Tags, X } from "lucide-react"
import React, { useState } from "react"
import { useTranslation } from "react-i18next"

import type { ActiveProfile } from "@/components/CommandLibrary/commandLibraryUtils"
import { CommandEditorDialog } from "@/components/CommandLibrary/CommandEditorDialog"
import { CommandListPane } from "@/components/CommandLibrary/CommandListPane"
import { CommandDetailPane } from "@/components/CommandLibrary/CommandDetailPane"
import { TagManagerDialog } from "@/components/CommandLibrary/TagManagerDialog"
import { useCommandLibraryData } from "@/components/CommandLibrary/useCommandLibraryData"
import { useCommandInsertion } from "@/components/CommandLibrary/useCommandInsertion"
import { CommandVariablesDialog } from "@/components/CommandVariablesDialog"
import { Button } from "@/components/ui/button"
import { Dialog, DialogContent, DialogDescription, DialogTitle } from "@/components/ui/dialog"
import { useConfirmDialog } from "@/components/ui/app-dialog"
import { useToast } from "@/hooks/use-toast"
import type { RecentCommand, SavedCommand } from "@/types/command"

interface CommandLibraryProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  activeProfile?: ActiveProfile
  canInsert: boolean
  onInsert: (command: SavedCommand) => Promise<boolean>
  onInsertRecent: (commandText: string) => Promise<boolean>
  recentCommands: RecentCommand[]
  initialSearchQuery?: string
}

export const CommandLibrary: React.FC<CommandLibraryProps> = ({
  open,
  onOpenChange,
  activeProfile,
  canInsert,
  onInsert,
  onInsertRecent,
  recentCommands,
  initialSearchQuery = "",
}) => {
  const { t } = useTranslation()
  const { toast } = useToast()
  const { confirm, ConfirmDialog } = useConfirmDialog()
  const [tagManagerOpen, setTagManagerOpen] = useState(false)
  const {
    searchInputRef,
    commands,
    setCommands,
    selectedId,
    setSelectedId,
    searchQuery,
    setSearchQuery,
    mode,
    setMode,
    isLoading,
    loadError,
    favoritePendingId,
    mobileDetailOpen,
    setMobileDetailOpen,
    selectedRecentId,
    setSelectedRecentId,
    recentPendingId,
    tags,
    setTags,
    loadCommands,
    filteredCommands,
    filteredRecentCommands,
    availableTags,
    selectedCommand,
    selectedRecent,
    selectedVariables,
    hasSelectedItem,
    handleSaved,
    handleFavorite,
    handleRecentFavorite,
    handleDelete,
    isFiltering,
  } = useCommandLibraryData({
    open,
    initialSearchQuery,
    recentCommands,
    onOpenChange,
    confirm,
    toast,
    t,
  })
  const {
    editorOpen,
    editingCommand,
    isInserting,
    setIsInserting,
    fillCommand,
    openEditor,
    handleEditorOpenChange,
    handleInsert,
    handleFillCancel,
    handleFillSubmit,
  } = useCommandInsertion({ onOpenChange, onInsert, setCommands, confirm, t })

  return (
    <>
      <Dialog open={open} onOpenChange={onOpenChange}>
        <DialogContent
          showCloseButton={false}
          className="flex h-[min(720px,calc(100dvh-2rem))] w-[min(960px,calc(100vw-2rem))] max-w-none flex-col gap-0 overflow-hidden p-0 shadow-2xl sm:max-w-none"
        >
          <header className="border-border/80 bg-muted/20 flex min-h-14 items-center justify-between gap-2 border-b px-3 py-2 sm:min-h-16 sm:gap-4 sm:px-5 sm:py-3">
            <div className="flex min-w-0 items-center gap-3">
              <div className="bg-muted text-muted-foreground hidden size-9 shrink-0 items-center justify-center rounded-md border sm:flex">
                <Library className="size-4" />
              </div>
              <div className="min-w-0">
                <DialogTitle className="text-base">{t("commandLibrary.title")}</DialogTitle>
                <DialogDescription className="mt-1 hidden truncate sm:block">
                  {t("commandLibrary.description")}
                </DialogDescription>
              </div>
            </div>
            <div className="flex shrink-0 items-center gap-2">
              <Button
                type="button"
                variant="outline"
                size="sm"
                aria-label={t("commandLibrary.tags.manage")}
                onClick={() => setTagManagerOpen(true)}
              >
                <Tags />
                <span className="hidden sm:inline">{t("commandLibrary.tags.manage")}</span>
              </Button>
              <Button
                type="button"
                size="sm"
                aria-label={t("commandLibrary.newCommand")}
                onClick={() => {
                  openEditor(null)
                }}
              >
                <Plus />
                <span className="hidden sm:inline">{t("commandLibrary.newCommand")}</span>
              </Button>
              <Button
                type="button"
                variant="ghost"
                size="icon-sm"
                aria-label={t("common.close")}
                onClick={() => onOpenChange(false)}
              >
                <X />
              </Button>
            </div>
          </header>

          <div className="grid min-h-0 flex-1 grid-cols-1 md:grid-cols-[minmax(280px,0.82fr)_minmax(0,1.18fr)]">
            <CommandListPane
              mobileDetailOpen={mobileDetailOpen}
              hasSelectedItem={hasSelectedItem}
              searchInputRef={searchInputRef}
              searchQuery={searchQuery}
              setSearchQuery={setSearchQuery}
              mode={mode}
              setMode={setMode}
              isLoading={isLoading}
              loadError={loadError}
              loadCommands={loadCommands}
              filteredRecentCommands={filteredRecentCommands}
              filteredCommands={filteredCommands}
              isFiltering={isFiltering}
              selectedId={selectedId}
              setSelectedId={setSelectedId}
              setMobileDetailOpen={setMobileDetailOpen}
              favoritePendingId={favoritePendingId}
              handleFavorite={handleFavorite}
              commands={commands}
              selectedRecentId={selectedRecentId}
              setSelectedRecentId={setSelectedRecentId}
              recentPendingId={recentPendingId}
              handleRecentFavorite={handleRecentFavorite}
              openEditor={openEditor}
            />

            <CommandDetailPane
              mobileDetailOpen={mobileDetailOpen}
              hasSelectedItem={hasSelectedItem}
              mode={mode}
              selectedRecent={selectedRecent}
              setMobileDetailOpen={setMobileDetailOpen}
              recentPendingId={recentPendingId}
              handleRecentFavorite={handleRecentFavorite}
              selectedCommand={selectedCommand}
              handleDelete={handleDelete}
              selectedVariables={selectedVariables}
              isInserting={isInserting}
              setIsInserting={setIsInserting}
              openEditor={openEditor}
              handleInsert={handleInsert}
              canInsert={canInsert}
              onInsertRecent={onInsertRecent}
              onOpenChange={onOpenChange}
            />
          </div>
        </DialogContent>
      </Dialog>
      <TagManagerDialog
        tagManagerOpen={tagManagerOpen}
        setTagManagerOpen={setTagManagerOpen}
        tags={tags}
        setTags={setTags}
        loadCommands={loadCommands}
      />

      <CommandEditorDialog
        open={editorOpen}
        command={editingCommand}
        availableTags={availableTags}
        activeProfile={activeProfile}
        onOpenChange={handleEditorOpenChange}
        onSaved={handleSaved}
      />
      <CommandVariablesDialog
        command={fillCommand}
        isInserting={isInserting}
        onCancel={handleFillCancel}
        onSubmit={(commandText) => void handleFillSubmit(commandText)}
      />
      <ConfirmDialog />
    </>
  )
}
