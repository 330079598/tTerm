import React, { useMemo, useState } from "react"
import { Braces, FileCode2, Loader2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"
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
import { useSyncedState } from "@/hooks/useSyncedState"
import {
  activeVariables,
  initialVariableValues,
  parseChoiceOptions,
  renderCommandText,
  validateVariableValues,
  type VariableValueError,
} from "@/lib/commandVariables"
import type { SavedCommand } from "@/types/command"

interface CommandVariablesDialogProps {
  /** The command being inserted; the dialog is open while this is set. */
  command: SavedCommand | null
  isInserting: boolean
  onCancel: () => void
  /** Receives the command text with every variable filled in. */
  onSubmit: (commandText: string) => void
}

/** Asks for the values of a saved command's variables before it is inserted. */
export const CommandVariablesDialog: React.FC<CommandVariablesDialogProps> = ({
  command,
  isInserting,
  onCancel,
  onSubmit,
}) => {
  const { t } = useTranslation()
  const variables = useMemo(() => (command ? activeVariables(command) : []), [command])
  const initialValues = useMemo(() => initialVariableValues(variables), [variables])
  const [values, setValues] = useSyncedState(initialValues)
  const [errors, setErrors] = useState<Record<string, VariableValueError>>({})
  const [errorsFor, setErrorsFor] = useState(command)
  if (errorsFor !== command) {
    setErrorsFor(command)
    setErrors({})
  }

  if (!command) return null

  const setValue = (name: string, value: string) => {
    setValues((current) => ({ ...current, [name]: value }))
    if (errors[name]) {
      setErrors((current) => {
        const { [name]: _cleared, ...rest } = current
        return rest
      })
    }
  }

  const handleSubmit = (event: React.FormEvent) => {
    event.preventDefault()
    const nextErrors = validateVariableValues(variables, values)
    setErrors(nextErrors)
    const firstInvalid = variables.find((variable) => nextErrors[variable.name])
    if (firstInvalid) {
      document.getElementById(`command-variable-${firstInvalid.name}`)?.focus()
      return
    }
    onSubmit(renderCommandText(command.commandText, variables, values))
  }

  return (
    <Dialog open onOpenChange={(open) => !open && !isInserting && onCancel()}>
      <DialogContent className="max-h-[88vh] overflow-y-auto sm:max-w-lg">
        <form onSubmit={handleSubmit} className="space-y-5">
          <DialogHeader>
            <DialogTitle>{command.name}</DialogTitle>
            <DialogDescription>{t("commandLibrary.variables.fillDescription")}</DialogDescription>
          </DialogHeader>

          {variables.map((variable, index) => {
            const id = `command-variable-${variable.name}`
            const error = errors[variable.name]
            const shared = {
              id,
              "aria-invalid": Boolean(error),
              "aria-describedby": error ? `${id}-error` : undefined,
              "data-dialog-initial-focus": index === 0 ? "true" : undefined,
            }
            return (
              <div key={variable.name} className="space-y-2">
                <Label htmlFor={id}>
                  {variable.label}
                  {!variable.isRequired && (
                    <span className="text-muted-foreground ml-1.5 text-xs font-normal">
                      {t("commandLibrary.variables.optional")}
                    </span>
                  )}
                </Label>
                {variable.valueType === "choice" ? (
                  <Select
                    {...shared}
                    value={values[variable.name] ?? ""}
                    onChange={(event) => setValue(variable.name, event.target.value)}
                  >
                    {!variable.isRequired && (
                      <option value="">{t("commandLibrary.variables.noValue")}</option>
                    )}
                    {parseChoiceOptions(variable.optionsJson).map((option) => (
                      <option key={option} value={option}>
                        {option}
                      </option>
                    ))}
                  </Select>
                ) : (
                  <Input
                    {...shared}
                    type={variable.valueType === "secret" ? "password" : "text"}
                    inputMode={variable.valueType === "number" ? "decimal" : undefined}
                    autoComplete={variable.valueType === "secret" ? "new-password" : "off"}
                    autoCapitalize="none"
                    autoCorrect="off"
                    spellCheck={false}
                    className="font-mono"
                    value={values[variable.name] ?? ""}
                    onChange={(event) => setValue(variable.name, event.target.value)}
                  />
                )}
                {error && (
                  <p id={`${id}-error`} role="alert" className="text-destructive text-xs">
                    {t(`commandLibrary.variables.valueErrors.${error}`)}
                  </p>
                )}
              </div>
            )
          })}

          <div className="border-border/80 bg-muted/30 rounded-md border p-3 shadow-inner">
            <div className="text-muted-foreground mb-2 flex items-center gap-2 text-xs font-medium">
              <Braces className="size-3.5" />
              {t("commandLibrary.commandPreview")}
            </div>
            <pre className="text-foreground max-h-40 overflow-auto font-mono text-sm leading-6 break-all whitespace-pre-wrap">
              {renderCommandText(command.commandText, variables, values, { maskSecrets: true })}
            </pre>
          </div>

          <DialogFooter>
            <Button type="button" variant="outline" disabled={isInserting} onClick={onCancel}>
              {t("common.cancel")}
            </Button>
            <Button type="submit" disabled={isInserting}>
              {isInserting ? <Loader2 className="animate-spin" /> : <FileCode2 />}
              {isInserting ? t("commandLibrary.inserting") : t("commandLibrary.insert")}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  )
}
