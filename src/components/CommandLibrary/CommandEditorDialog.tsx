import { invoke } from "@tauri-apps/api/core"
import { Check, Loader2 } from "lucide-react"
import React, { useEffect, useMemo, useRef, useState } from "react"
import { useTranslation } from "react-i18next"

import {
  addCommandTags,
  parseCommandTags,
  type ActiveProfile,
} from "@/components/CommandLibrary/commandLibraryUtils"
import { CommandTagsField } from "@/components/CommandLibrary/CommandTagsField"
import {
  CommandVariablesEditor,
  DEFAULT_VARIABLE_DRAFT,
  draftFromVariable,
  validateVariableDraft,
  variableFromDraft,
  type VariableDraft,
  type VariableDraftError,
} from "@/components/CommandVariablesEditor"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import { useToast } from "@/hooks/use-toast"
import { extractPlaceholderNames } from "@/lib/commandVariables"
import type { CommandDraft, SaveCommandInput, SavedCommand } from "@/types/command"

export function CommandEditorDialog({
  open,
  command,
  draft,
  availableTags,
  activeProfile,
  onOpenChange,
  onSaved,
}: {
  open: boolean
  command: SavedCommand | null
  draft?: CommandDraft | null
  availableTags: string[]
  activeProfile?: ActiveProfile
  onOpenChange: (open: boolean) => void
  onSaved: (command: SavedCommand) => void
}) {
  const { t } = useTranslation()
  const { toast } = useToast()
  const nameInputRef = useRef<HTMLInputElement>(null)
  const [name, setName] = useState("")
  const [commandText, setCommandText] = useState("")
  const [description, setDescription] = useState("")
  const [selectedTags, setSelectedTags] = useState<string[]>([])
  const [tagDraft, setTagDraft] = useState("")
  const [scopeType, setScopeType] = useState<"global" | "profile">("global")
  const [isFavorite, setIsFavorite] = useState(false)
  const [confirmBeforeRun, setConfirmBeforeRun] = useState(false)
  const [variableDrafts, setVariableDrafts] = useState<Record<string, VariableDraft>>({})
  // Placeholders inserted as written. Those already in the text when the form
  // opens start here unless they were saved as variables, so existing commands
  // with template syntax keep working; ones typed afterwards become variables.
  const [literalNames, setLiteralNames] = useState<string[]>([])
  const [variableErrors, setVariableErrors] = useState<
    Record<string, VariableDraftError | undefined>
  >({})
  const [isSaving, setIsSaving] = useState(false)
  const [errors, setErrors] = useState<{ name?: string; commandText?: string }>({})
  const source = command ?? draft
  const profileScopeId = source?.scopeId ?? activeProfile?.id
  const profileScopeLabel =
    command?.scopeType === "profile" && command.scopeId !== activeProfile?.id
      ? t("commandLibrary.form.savedConnection")
      : activeProfile?.name

  // Reset the form whenever the dialog opens or its source changes.
  const [formOpen, setFormOpen] = useState(false)
  const [formSource, setFormSource] = useState(source)
  if (formOpen !== open || formSource !== source) {
    setFormOpen(open)
    setFormSource(source)
    if (open) {
      setName(source?.name ?? "")
      setCommandText(source?.commandText ?? "")
      setDescription(source?.description ?? "")
      setSelectedTags(parseCommandTags(source?.tags.join("\n") ?? ""))
      setTagDraft("")
      setScopeType(source?.scopeType ?? "global")
      setIsFavorite(source?.isFavorite ?? false)
      setConfirmBeforeRun(source?.confirmBeforeRun ?? false)
      const savedVariables = source?.variables ?? []
      setVariableDrafts(
        Object.fromEntries(
          savedVariables.map((variable) => [variable.name, draftFromVariable(variable)])
        )
      )
      setLiteralNames(
        extractPlaceholderNames(source?.commandText ?? "").filter(
          (placeholder) => !savedVariables.some((variable) => variable.name === placeholder)
        )
      )
      setVariableErrors({})
      setErrors({})
      setIsSaving(false)
    }
  }

  useEffect(() => {
    if (!open) return
    requestAnimationFrame(() => nameInputRef.current?.focus())
  }, [open, source])

  const placeholderNames = useMemo(() => extractPlaceholderNames(commandText), [commandText])
  const variableNames = placeholderNames.filter(
    (placeholder) => !literalNames.includes(placeholder)
  )

  const validate = () => {
    const nextErrors: typeof errors = {}
    if (!name.trim()) nextErrors.name = t("commandLibrary.form.nameRequired")
    if (!commandText.trim()) nextErrors.commandText = t("commandLibrary.form.commandRequired")
    setErrors(nextErrors)
    if (nextErrors.name) nameInputRef.current?.focus()

    const nextVariableErrors: typeof variableErrors = {}
    for (const variableName of variableNames) {
      const error = validateVariableDraft(variableDrafts[variableName] ?? DEFAULT_VARIABLE_DRAFT)
      if (error) nextVariableErrors[variableName] = error
    }
    setVariableErrors(nextVariableErrors)

    return Object.keys(nextErrors).length === 0 && Object.keys(nextVariableErrors).length === 0
  }

  const handleSubmit = async (event: React.FormEvent) => {
    event.preventDefault()
    if (!validate()) return

    const input: SaveCommandInput = {
      id: command?.id,
      name,
      commandText,
      description,
      tags: addCommandTags(selectedTags, tagDraft),
      scopeType,
      scopeId: scopeType === "profile" ? profileScopeId : undefined,
      isFavorite,
      confirmBeforeRun,
      variables: variableNames.map((variableName, position) =>
        variableFromDraft(
          variableName,
          variableDrafts[variableName] ?? DEFAULT_VARIABLE_DRAFT,
          position
        )
      ),
    }

    setIsSaving(true)
    try {
      const saved = await invoke<SavedCommand>("save_saved_command", { input })
      onSaved(saved)
      onOpenChange(false)
      toast({
        title: command
          ? t("commandLibrary.messages.updated")
          : t("commandLibrary.messages.created"),
      })
    } catch (error) {
      toast({
        title: t("commandLibrary.messages.saveFailed"),
        description: String(error),
        variant: "destructive",
      })
    } finally {
      setIsSaving(false)
    }
  }

  return (
    <Dialog open={open} onOpenChange={(nextOpen) => !isSaving && onOpenChange(nextOpen)}>
      <DialogContent className="max-h-[88vh] overflow-y-auto sm:max-w-xl">
        <form onSubmit={handleSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>
              {command ? t("commandLibrary.editTitle") : t("commandLibrary.newTitle")}
            </DialogTitle>
            <DialogDescription>{t("commandLibrary.form.description")}</DialogDescription>
          </DialogHeader>

          <div className="space-y-2">
            <Label htmlFor="saved-command-name">{t("commandLibrary.form.name")}</Label>
            <Input
              ref={nameInputRef}
              id="saved-command-name"
              value={name}
              maxLength={120}
              aria-invalid={Boolean(errors.name)}
              aria-describedby={errors.name ? "saved-command-name-error" : undefined}
              onBlur={() => {
                if (!name.trim()) {
                  setErrors((current) => ({
                    ...current,
                    name: t("commandLibrary.form.nameRequired"),
                  }))
                }
              }}
              onChange={(event) => {
                setName(event.target.value)
                if (errors.name) setErrors((current) => ({ ...current, name: undefined }))
              }}
            />
            {errors.name && (
              <p id="saved-command-name-error" role="alert" className="text-destructive text-xs">
                {errors.name}
              </p>
            )}
          </div>

          <div className="space-y-2">
            <Label htmlFor="saved-command-text">{t("commandLibrary.form.command")}</Label>
            <Textarea
              id="saved-command-text"
              value={commandText}
              maxLength={65_536}
              className="min-h-36 font-mono leading-6"
              aria-invalid={Boolean(errors.commandText)}
              aria-describedby={
                errors.commandText ? "saved-command-text-error" : "saved-command-text-hint"
              }
              onBlur={() => {
                if (!commandText.trim()) {
                  setErrors((current) => ({
                    ...current,
                    commandText: t("commandLibrary.form.commandRequired"),
                  }))
                }
              }}
              onChange={(event) => {
                setCommandText(event.target.value)
                if (errors.commandText) {
                  setErrors((current) => ({ ...current, commandText: undefined }))
                }
              }}
            />
            {errors.commandText ? (
              <p id="saved-command-text-error" role="alert" className="text-destructive text-xs">
                {errors.commandText}
              </p>
            ) : (
              <p id="saved-command-text-hint" className="text-muted-foreground text-xs">
                {t("commandLibrary.form.commandHint")}
              </p>
            )}
          </div>

          <CommandVariablesEditor
            names={placeholderNames}
            drafts={variableDrafts}
            literalNames={literalNames}
            errors={variableErrors}
            onDraftChange={(variableName, patch) => {
              setVariableDrafts((current) => ({
                ...current,
                [variableName]: {
                  ...(current[variableName] ?? DEFAULT_VARIABLE_DRAFT),
                  ...patch,
                },
              }))
              if (variableErrors[variableName]) {
                setVariableErrors((current) => ({ ...current, [variableName]: undefined }))
              }
            }}
            onLiteralChange={(variableName, literal) =>
              setLiteralNames((current) =>
                literal
                  ? [...current, variableName]
                  : current.filter((candidate) => candidate !== variableName)
              )
            }
          />

          <div className="space-y-2">
            <Label htmlFor="saved-command-description">
              {t("commandLibrary.form.descriptionLabel")}
            </Label>
            <Textarea
              id="saved-command-description"
              value={description}
              className="min-h-20"
              onChange={(event) => setDescription(event.target.value)}
            />
          </div>

          <CommandTagsField
            selectedTags={selectedTags}
            setSelectedTags={setSelectedTags}
            tagDraft={tagDraft}
            setTagDraft={setTagDraft}
            availableTags={availableTags}
          />

          {profileScopeId && (
            <div className="space-y-2">
              <Label htmlFor="saved-command-scope">{t("commandLibrary.form.scope")}</Label>
              <Select
                id="saved-command-scope"
                value={scopeType}
                onChange={(event) => setScopeType(event.target.value as "global" | "profile")}
              >
                <option value="global">{t("commandLibrary.scope.global")}</option>
                <option value="profile">
                  {t("commandLibrary.scope.connection", { name: profileScopeLabel })}
                </option>
              </Select>
            </div>
          )}

          <label className="hover:bg-muted/40 flex min-h-11 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 transition-colors">
            <Checkbox
              checked={isFavorite}
              onCheckedChange={setIsFavorite}
              aria-label={t("commandLibrary.form.favorite")}
            />
            <span className="flex min-w-0 flex-col gap-1">
              <span className="text-sm font-medium">{t("commandLibrary.form.favorite")}</span>
              <span className="text-muted-foreground text-xs">
                {t("commandLibrary.form.favoriteHint")}
              </span>
            </span>
          </label>

          <label className="hover:bg-muted/40 flex min-h-11 cursor-pointer items-center gap-3 rounded-md border px-3 py-2 transition-colors">
            <Checkbox
              checked={confirmBeforeRun}
              onCheckedChange={setConfirmBeforeRun}
              aria-label={t("commandLibrary.form.confirmBeforeRun")}
            />
            <span className="flex min-w-0 flex-col gap-1">
              <span className="text-sm font-medium">
                {t("commandLibrary.form.confirmBeforeRun")}
              </span>
              <span className="text-muted-foreground text-xs">
                {t("commandLibrary.form.confirmBeforeRunHint")}
              </span>
            </span>
          </label>

          <DialogFooter>
            <Button
              type="button"
              variant="outline"
              disabled={isSaving}
              onClick={() => onOpenChange(false)}
            >
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={isSaving}>
              {isSaving ? <Loader2 className="animate-spin" /> : <Check />}
              {isSaving ? t("common.saving") : t("common.save")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
