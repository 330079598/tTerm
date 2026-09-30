import React from "react"
import { useTranslation } from "react-i18next"

import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Select } from "@/components/ui/select"
import { Textarea } from "@/components/ui/textarea"
import {
  formatPlaceholder,
  parseChoiceOptions,
  parseChoiceOptionsText,
  PLACEHOLDER_EXAMPLE,
} from "@/lib/commandVariables"
import type { SavedCommandVariable } from "@/types/command"

const VALUE_TYPES = ["text", "number", "choice", "secret"] as const

/** A variable definition as edited in the form. */
export interface VariableDraft {
  label: string
  valueType: SavedCommandVariable["valueType"]
  defaultValue: string
  /** Choice options, one per line. */
  optionsText: string
  isRequired: boolean
}

export const DEFAULT_VARIABLE_DRAFT: VariableDraft = {
  label: "",
  valueType: "text",
  defaultValue: "",
  optionsText: "",
  isRequired: true,
}

export type VariableDraftError = "options" | "number"

export function draftFromVariable(variable: SavedCommandVariable): VariableDraft {
  return {
    label: variable.label === variable.name ? "" : variable.label,
    valueType: variable.valueType,
    defaultValue: variable.defaultValue ?? "",
    optionsText: parseChoiceOptions(variable.optionsJson).join("\n"),
    isRequired: variable.isRequired,
  }
}

export function validateVariableDraft(draft: VariableDraft): VariableDraftError | null {
  if (draft.valueType === "choice" && parseChoiceOptionsText(draft.optionsText).length === 0) {
    return "options"
  }
  if (
    draft.valueType === "number" &&
    draft.defaultValue.trim() !== "" &&
    !Number.isFinite(Number(draft.defaultValue))
  ) {
    return "number"
  }
  return null
}

export function variableFromDraft(
  name: string,
  draft: VariableDraft,
  position: number
): SavedCommandVariable {
  const options = draft.valueType === "choice" ? parseChoiceOptionsText(draft.optionsText) : []
  const defaultValue = draft.valueType === "secret" ? "" : draft.defaultValue.trim()
  return {
    name,
    label: draft.label.trim() || name,
    valueType: draft.valueType,
    defaultValue:
      defaultValue && (draft.valueType !== "choice" || options.includes(defaultValue))
        ? defaultValue
        : undefined,
    optionsJson: draft.valueType === "choice" ? JSON.stringify(options) : undefined,
    isRequired: draft.isRequired,
    position,
  }
}

interface CommandVariablesEditorProps {
  /** Placeholder names found in the command text, in order. */
  names: string[]
  drafts: Record<string, VariableDraft>
  /** Placeholders to insert as literal text instead of asking for a value. */
  literalNames: string[]
  errors: Record<string, VariableDraftError | undefined>
  onDraftChange: (name: string, patch: Partial<VariableDraft>) => void
  onLiteralChange: (name: string, literal: boolean) => void
}

export const CommandVariablesEditor: React.FC<CommandVariablesEditorProps> = ({
  names,
  drafts,
  literalNames,
  errors,
  onDraftChange,
  onLiteralChange,
}) => {
  const { t } = useTranslation()

  return (
    <div className="space-y-2">
      <Label>{t("commandLibrary.variables.title")}</Label>
      <p className="text-muted-foreground text-xs leading-5">
        {t("commandLibrary.variables.hint", { example: PLACEHOLDER_EXAMPLE })}
      </p>
      {names.map((name) => {
        const draft = drafts[name] ?? DEFAULT_VARIABLE_DRAFT
        const enabled = !literalNames.includes(name)
        const error = enabled ? errors[name] : undefined
        const options =
          draft.valueType === "choice" ? parseChoiceOptionsText(draft.optionsText) : []
        const fieldId = `saved-command-variable-${name}`
        return (
          <div key={name} className="border-input space-y-3 rounded-md border px-3 py-2.5">
            <label className="flex cursor-pointer items-center gap-3">
              <Checkbox
                checked={enabled}
                onCheckedChange={(checked) => onLiteralChange(name, !checked)}
                aria-label={t("commandLibrary.variables.askFor", {
                  placeholder: formatPlaceholder(name),
                })}
              />
              <code className="bg-muted rounded-sm px-1.5 py-0.5 font-mono text-xs">
                {formatPlaceholder(name)}
              </code>
              <span className="text-muted-foreground min-w-0 truncate text-xs">
                {enabled
                  ? t("commandLibrary.variables.askOnInsert")
                  : t("commandLibrary.variables.keepLiteral")}
              </span>
            </label>

            {enabled && (
              <>
                <div className="grid grid-cols-1 gap-3 sm:grid-cols-2">
                  <div className="space-y-1.5">
                    <Label htmlFor={`${fieldId}-label`} className="text-xs font-normal">
                      {t("commandLibrary.variables.label")}
                    </Label>
                    <Input
                      id={`${fieldId}-label`}
                      value={draft.label}
                      maxLength={120}
                      placeholder={name}
                      onChange={(event) => onDraftChange(name, { label: event.target.value })}
                    />
                  </div>
                  <div className="space-y-1.5">
                    <Label htmlFor={`${fieldId}-type`} className="text-xs font-normal">
                      {t("commandLibrary.variables.type")}
                    </Label>
                    <Select
                      id={`${fieldId}-type`}
                      value={draft.valueType}
                      onChange={(event) =>
                        onDraftChange(name, {
                          valueType: event.target.value as VariableDraft["valueType"],
                        })
                      }
                    >
                      {VALUE_TYPES.map((valueType) => (
                        <option key={valueType} value={valueType}>
                          {t(`commandLibrary.variables.types.${valueType}`)}
                        </option>
                      ))}
                    </Select>
                  </div>
                </div>

                {draft.valueType === "choice" && (
                  <div className="space-y-1.5">
                    <Label htmlFor={`${fieldId}-options`} className="text-xs font-normal">
                      {t("commandLibrary.variables.options")}
                    </Label>
                    <Textarea
                      id={`${fieldId}-options`}
                      value={draft.optionsText}
                      className="min-h-16 font-mono"
                      placeholder={t("commandLibrary.variables.optionsPlaceholder")}
                      aria-invalid={error === "options"}
                      onChange={(event) => onDraftChange(name, { optionsText: event.target.value })}
                    />
                  </div>
                )}

                {draft.valueType === "secret" ? (
                  <p className="text-muted-foreground text-xs leading-5">
                    {t("commandLibrary.variables.secretHint")}
                  </p>
                ) : (
                  <div className="space-y-1.5">
                    <Label htmlFor={`${fieldId}-default`} className="text-xs font-normal">
                      {t("commandLibrary.variables.defaultValue")}
                    </Label>
                    {draft.valueType === "choice" ? (
                      <Select
                        id={`${fieldId}-default`}
                        value={options.includes(draft.defaultValue) ? draft.defaultValue : ""}
                        onChange={(event) =>
                          onDraftChange(name, { defaultValue: event.target.value })
                        }
                      >
                        <option value="">{t("commandLibrary.variables.noDefault")}</option>
                        {options.map((option) => (
                          <option key={option} value={option}>
                            {option}
                          </option>
                        ))}
                      </Select>
                    ) : (
                      <Input
                        id={`${fieldId}-default`}
                        value={draft.defaultValue}
                        inputMode={draft.valueType === "number" ? "decimal" : undefined}
                        className="font-mono"
                        aria-invalid={error === "number"}
                        onChange={(event) =>
                          onDraftChange(name, { defaultValue: event.target.value })
                        }
                      />
                    )}
                  </div>
                )}

                {error && (
                  <p role="alert" className="text-destructive text-xs">
                    {t(`commandLibrary.variables.errors.${error}`)}
                  </p>
                )}

                <label className="flex cursor-pointer items-center gap-3">
                  <Checkbox
                    checked={draft.isRequired}
                    onCheckedChange={(checked) => onDraftChange(name, { isRequired: checked })}
                    aria-label={t("commandLibrary.variables.required")}
                  />
                  <span className="text-xs">{t("commandLibrary.variables.required")}</span>
                </label>
              </>
            )}
          </div>
        )
      })}
    </div>
  )
}
