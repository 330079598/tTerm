import { useState } from "react"
import { useTranslation } from "react-i18next"

import { ShellPathInput } from "@/components/ShellPathInput"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import type { LocalShell } from "@/lib/appConfig"
import { programName } from "@/lib/localShells"

interface LocalShellEditorProps {
  /** The shell being edited, or a new one with its id and any path to start from. */
  shell: LocalShell
  isDefault: boolean
  onSave: (shell: LocalShell, makeDefault: boolean) => void
  onCancel: () => void
}

/** The form for adding a shell or changing one: its program, name and arguments. */
export function LocalShellEditor({ shell, isDefault, onSave, onCancel }: LocalShellEditorProps) {
  const { t } = useTranslation()
  const [path, setPath] = useState(shell.path)
  const [name, setName] = useState(shell.name)
  const [args, setArgs] = useState(shell.args)
  const [makeDefault, setMakeDefault] = useState(false)
  const fieldId = (field: string) => `local-shell-${shell.id}-${field}`

  const submit = (event: React.FormEvent) => {
    event.preventDefault()
    if (!path.trim()) return
    onSave({ id: shell.id, name: name.trim(), path: path.trim(), args: args.trim() }, makeDefault)
  }

  return (
    <form className="bg-muted/40 space-y-3 rounded-md p-3" onSubmit={submit}>
      <div className="space-y-1.5">
        <Label htmlFor={fieldId("path")}>{t("localShells.program")}</Label>
        <ShellPathInput id={fieldId("path")} value={path} onChange={setPath} required />
      </div>
      <div className="grid gap-3 sm:grid-cols-2">
        <div className="space-y-1.5">
          <Label htmlFor={fieldId("name")}>{t("localShells.name")}</Label>
          <Input
            id={fieldId("name")}
            value={name}
            onChange={(event) => setName(event.target.value)}
            placeholder={programName(path) || t("localShells.namePlaceholder")}
          />
        </div>
        <div className="space-y-1.5">
          <Label htmlFor={fieldId("args")}>{t("localShells.args")}</Label>
          <Input
            id={fieldId("args")}
            value={args}
            onChange={(event) => setArgs(event.target.value)}
            placeholder="--login -i"
          />
        </div>
      </div>
      <div className="flex flex-wrap items-center justify-between gap-3">
        {isDefault ? (
          <span className="text-muted-foreground text-xs">{t("localShells.isDefault")}</span>
        ) : (
          <label className="flex items-center gap-2 text-sm">
            <Checkbox checked={makeDefault} onCheckedChange={setMakeDefault} />
            {t("localShells.makeDefault")}
          </label>
        )}
        <div className="flex gap-2">
          <Button type="button" variant="ghost" size="sm" onClick={onCancel}>
            {t("localShells.cancel")}
          </Button>
          <Button type="submit" size="sm" disabled={!path.trim()}>
            {t("localShells.save")}
          </Button>
        </div>
      </div>
    </form>
  )
}
