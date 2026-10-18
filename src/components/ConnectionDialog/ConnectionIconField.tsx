import React from "react"
import { useTranslation } from "react-i18next"

import type { ConnectionForm } from "@/components/ConnectionDialog/types"
import { TabIconGlyph } from "@/components/TabIcon"
import { TabIconPicker } from "@/components/TabIconPicker"
import { Button } from "@/components/ui/button"
import { Label } from "@/components/ui/label"
import { useConfig } from "@/contexts/ConfigContext"
import { distroName } from "@/lib/distro"
import { applyTabIconStyle, sshTabIcon } from "@/lib/tabIcons"

interface ConnectionIconFieldProps {
  form: ConnectionForm
  setForm: React.Dispatch<React.SetStateAction<ConnectionForm>>
}

/** The icon a connection's tabs show, picked behind a button since the picker is tall. */
export function ConnectionIconField({ form, setForm }: ConnectionIconFieldProps) {
  const { t } = useTranslation()
  const { config } = useConfig()
  const [open, setOpen] = React.useState(false)
  if (!config.tab_icons.enabled) return null

  const inherited = sshTabIcon(form.distro)
  return (
    <div>
      <Label className="mb-1.5 block">{t("connection.tabIcon")}</Label>
      <div className="flex items-center gap-3">
        <TabIconGlyph icon={applyTabIconStyle(inherited, form.tabIcon)} size={16} />
        <span className="text-muted-foreground min-w-0 flex-1 truncate text-xs">
          {form.tabIcon
            ? t("connection.tabIconChosen")
            : form.distro
              ? t("connection.tabIconDistro", { name: distroName(form.distro) })
              : t("connection.tabIconUnknown")}
        </span>
        <Button
          type="button"
          variant="outline"
          size="xs"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
        >
          {open ? t("tabIcons.done") : t("tabIcons.change")}
        </Button>
      </div>
      {open && (
        <div className="mt-3">
          <TabIconPicker
            value={form.tabIcon}
            inherited={inherited}
            resetLabel={t("tabIcons.resetSshTab")}
            onChange={(tabIcon) => setForm((current) => ({ ...current, tabIcon }))}
          />
        </div>
      )}
    </div>
  )
}
