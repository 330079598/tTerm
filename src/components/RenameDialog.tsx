import React from "react"
import { useTranslation } from "react-i18next"
import { Edit2 } from "lucide-react"
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from "@/components/ui/dialog"
import { TabIconPicker } from "@/components/TabIconPicker"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { useConfig } from "@/contexts/ConfigContext"
import { inheritedTabIcon, type TabIconStyle } from "@/lib/tabIcons"
import { Label } from "@/components/ui/label"
import type { Tab } from "@/types/tab"

/** A terminal tab's own icon, edited beside its name. */
interface TabIconEdit {
  tab: Pick<Tab, "type" | "shellName" | "distroId" | "connection">
  value: TabIconStyle | undefined
  onChange: (value: TabIconStyle | undefined) => void
}

interface RenameDialogProps {
  isOpen: boolean
  currentName: string
  onConfirm: (newName: string) => void
  onClose: () => void
  /** Given for a local tab the user named: names it automatically again. */
  onUseAutoTitle?: () => void
  /** Given for a terminal tab, local or SSH, whose icon can be chosen too. */
  icon?: TabIconEdit
}

function sameStyle(a: TabIconStyle | undefined, b: TabIconStyle | undefined) {
  return a?.icon === b?.icon && a?.color === b?.color
}

export const RenameDialog: React.FC<RenameDialogProps> = ({ isOpen, onClose, ...props }) => {
  const { config } = useConfig()
  const showIcon = props.icon !== undefined && config.tab_icons.enabled

  return (
    <Dialog open={isOpen} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className={showIcon ? "sm:max-w-md" : "sm:max-w-sm"}>
        <RenameForm {...props} onClose={onClose} showIcon={showIcon} />
      </DialogContent>
    </Dialog>
  )
}

// Mounted while the dialog is open, so each opening starts from the tab as it is.
function RenameForm({
  currentName,
  onConfirm,
  onClose,
  onUseAutoTitle,
  icon,
  showIcon,
}: Omit<RenameDialogProps, "isOpen"> & { showIcon: boolean }) {
  const { t } = useTranslation()
  const { config } = useConfig()
  const [name, setName] = React.useState(currentName)
  const [iconStyle, setIconStyle] = React.useState(icon?.value)

  const commitIcon = () => {
    if (icon && !sameStyle(iconStyle, icon.value)) icon.onChange(iconStyle)
  }

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    const trimmedName = name.trim()
    if (trimmedName && trimmedName !== currentName) {
      onConfirm(trimmedName)
    }
    commitIcon()
    onClose()
  }

  return (
    <>
      <DialogHeader>
        <DialogTitle className="flex items-center gap-2">
          <Edit2 size={16} />
          {icon ? t("rename.editTitle") : t("rename.title")}
        </DialogTitle>
      </DialogHeader>

      <form onSubmit={handleSubmit}>
        <div className="py-2">
          <Label htmlFor="rename-input" className="mb-2 block">
            {t("rename.newName")}
          </Label>
          <Input
            id="rename-input"
            value={name}
            onChange={(e) => setName(e.target.value)}
            placeholder={currentName}
            autoFocus
          />
        </div>

        {showIcon && icon && (
          <div className="py-2">
            <TabIconPicker
              value={iconStyle}
              inherited={inheritedTabIcon(icon.tab, config.tab_icons)}
              resetLabel={t(icon.tab.type === "ssh" ? "tabIcons.resetSshTab" : "tabIcons.resetTab")}
              onChange={setIconStyle}
            />
            {icon.tab.type === "ssh" && icon.tab.connection?.profileId && (
              <p className="text-muted-foreground mt-2 text-xs">{t("tabIcons.connectionShared")}</p>
            )}
          </div>
        )}

        <DialogFooter className="mt-4">
          {onUseAutoTitle && (
            <Button
              type="button"
              variant="ghost"
              className="sm:mr-auto"
              onClick={() => {
                onUseAutoTitle()
                commitIcon()
                onClose()
              }}
            >
              {t("rename.useAutoTitle")}
            </Button>
          )}
          <Button type="button" variant="ghost" onClick={onClose}>
            {t("rename.cancel")}
          </Button>
          <Button type="submit" disabled={!name.trim()}>
            {icon ? t("rename.save") : t("rename.confirm")}
          </Button>
        </DialogFooter>
      </form>
    </>
  )
}
