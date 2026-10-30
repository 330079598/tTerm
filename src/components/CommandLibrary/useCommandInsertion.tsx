import type { TFunction } from "i18next"
import { invoke } from "@tauri-apps/api/core"
import { useState } from "react"

import type { CommandLibraryData } from "@/components/CommandLibrary/useCommandLibraryData"
import type { useConfirmDialog } from "@/components/ui/app-dialog"
import { insertIntoComposer } from "@/lib/commandComposer"
import { activeVariables } from "@/lib/commandVariables"
import type { SavedCommand } from "@/types/command"

interface CommandInsertionOptions {
  onOpenChange: (open: boolean) => void
  onInsert: (command: SavedCommand) => Promise<boolean>
  setCommands: CommandLibraryData["setCommands"]
  confirm: ReturnType<typeof useConfirmDialog>["confirm"]
  t: TFunction
}

/**
 * Where an inserted command goes: typed into the terminal, or into the
 * compose bar to be looked over, added to and sent from there.
 */
export type InsertDestination = "terminal" | "composer"

/**
 * Inserting a command into the terminal (asking for its variables or a
 * confirmation first when it has them) or into the compose bar, and opening
 * the command editor.
 */
export function useCommandInsertion({
  onOpenChange,
  onInsert,
  setCommands,
  confirm,
  t,
}: CommandInsertionOptions) {
  const [editorOpen, setEditorOpen] = useState(false)
  const [editingCommand, setEditingCommand] = useState<SavedCommand | null>(null)
  const [isInserting, setIsInserting] = useState(false)
  const [fillCommand, setFillCommand] = useState<SavedCommand | null>(null)
  const [fillDestination, setFillDestination] = useState<InsertDestination>("terminal")

  const openEditor = (command: SavedCommand | null) => {
    setEditingCommand(command)
    onOpenChange(false)
    setEditorOpen(true)
  }

  const handleEditorOpenChange = (nextOpen: boolean) => {
    setEditorOpen(nextOpen)
    if (!nextOpen) onOpenChange(true)
  }

  /** Inserts `commandText` (the command's text, variables filled in) and counts the use. */
  const insertCommandText = async (
    command: SavedCommand,
    commandText: string,
    destination: InsertDestination
  ) => {
    setIsInserting(true)
    try {
      if (destination === "composer") insertIntoComposer(commandText)
      else if (!(await onInsert({ ...command, commandText }))) return false
      void invoke("record_saved_command_use", { id: command.id }).catch(console.error)
      setCommands((current) =>
        current.map((candidate) =>
          candidate.id === command.id
            ? { ...candidate, useCount: candidate.useCount + 1, lastUsedAt: Date.now() }
            : candidate
        )
      )
      return true
    } finally {
      setIsInserting(false)
    }
  }

  const handleInsert = async (
    command: SavedCommand,
    destination: InsertDestination = "terminal"
  ) => {
    // The variables dialog shows the final command, so it doubles as the confirmation.
    if (activeVariables(command).length > 0) {
      onOpenChange(false)
      setFillDestination(destination)
      setFillCommand(command)
      return
    }

    // In the compose bar the command waits to be sent, which is its confirmation.
    if (command.confirmBeforeRun && destination === "terminal") {
      onOpenChange(false)
      const confirmed = await confirm({
        title: t("commandLibrary.confirmInsertTitle"),
        description: (
          <>
            {t("commandLibrary.confirmInsertDescription", { name: command.name })}
            <code className="bg-muted text-foreground mt-2 block max-h-40 overflow-auto rounded-md px-2 py-1.5 font-mono text-xs break-all whitespace-pre-wrap">
              {command.commandText}
            </code>
          </>
        ),
        confirmText: t("commandLibrary.insert"),
        cancelText: t("common.cancel"),
        defaultAction: "cancel",
      })
      if (!confirmed || !(await insertCommandText(command, command.commandText, destination))) {
        onOpenChange(true)
      }
      return
    }

    if (await insertCommandText(command, command.commandText, destination)) onOpenChange(false)
  }

  const handleFillCancel = () => {
    setFillCommand(null)
    onOpenChange(true)
  }

  const handleFillSubmit = async (commandText: string) => {
    if (!fillCommand) return
    // On failure the dialog stays open so the entered values are not lost.
    if (await insertCommandText(fillCommand, commandText, fillDestination)) setFillCommand(null)
  }

  return {
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
  }
}

export type CommandInsertion = ReturnType<typeof useCommandInsertion>
