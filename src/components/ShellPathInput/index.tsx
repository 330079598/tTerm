import { open as openFileDialog } from "@tauri-apps/plugin-dialog"
import { CircleAlert, CircleCheck, FolderOpen } from "lucide-react"
import { useTranslation } from "react-i18next"

import { useShellPathCheck } from "@/components/ShellPathInput/useShellPathCheck"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { getDetectedPlatform } from "@/lib/uiPlatform"

interface ShellPathInputProps {
  id: string
  value: string
  onChange: (path: string) => void
  required?: boolean
}

/**
 * A shell program's path, typed or picked with a file dialog, with the
 * program it starts on this machine shown underneath as it is typed.
 */
export function ShellPathInput({ id, value, onChange, required }: ShellPathInputProps) {
  const { t } = useTranslation()
  const check = useShellPathCheck(value)
  const windows = getDetectedPlatform() === "windows"

  const browse = async () => {
    const selected = await openFileDialog({
      multiple: false,
      directory: false,
      title: t("localShells.pickProgram"),
      filters: windows
        ? [{ name: t("localShells.programs"), extensions: ["exe", "bat", "cmd"] }]
        : undefined,
    }).catch(() => null)
    if (typeof selected === "string") onChange(selected)
  }

  return (
    <div className="space-y-1.5">
      <div className="flex gap-2">
        <Input
          id={id}
          value={value}
          onChange={(event) => onChange(event.target.value)}
          placeholder={windows ? "C:\\msys64\\usr\\bin\\bash.exe" : "/opt/homebrew/bin/fish"}
          required={required}
        />
        <Button type="button" variant="outline" onClick={() => void browse()}>
          <FolderOpen size={14} />
          {t("localShells.browse")}
        </Button>
      </div>
      {check.status === "checking" && (
        <p className="text-muted-foreground text-xs">{t("localShells.checking")}</p>
      )}
      {check.status === "found" && (
        <p className="text-muted-foreground flex min-w-0 items-center gap-1.5 text-xs">
          <CircleCheck size={12} className="shrink-0 text-emerald-500" />
          <span className="truncate">{t("localShells.starts", { program: check.program })}</span>
        </p>
      )}
      {check.status === "missing" && (
        <p className="text-destructive flex items-center gap-1.5 text-xs">
          <CircleAlert size={12} className="shrink-0" />
          {t("localShells.notFound")}
        </p>
      )}
    </div>
  )
}
