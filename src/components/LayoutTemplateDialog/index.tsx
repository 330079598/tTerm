import "@/components/LayoutTemplateDialog/LayoutTemplateDialog.css"
import React from "react"
import { LayoutTemplate, Trash2 } from "lucide-react"
import { useTranslation } from "react-i18next"

import { LayoutShapePreview } from "@/components/LayoutTemplateDialog/LayoutShapePreview"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { useConfig } from "@/contexts/ConfigContext"
import { useSettingsSave } from "@/hooks/useSettingsSave"
import { isImeKeyEvent } from "@/lib/ime"
import {
  BUILTIN_PANE_TEMPLATES,
  countTemplatePanes,
  shapeOfTree,
  type LayoutShape,
  type PaneLayoutTemplate,
} from "@/lib/paneTemplates"
import { getPaneIds, type PaneNode } from "@/lib/tabGroups"

export interface LayoutTemplateTarget {
  /** The pane the layout is applied around; panes it adds copy this one. */
  tabId: string
  /** The split tree of the tab of the bar holding it. */
  root: PaneNode
  /** The pane is a session that can be copied into the panes a layout adds. */
  canCopy: boolean
}

interface LayoutTemplateDialogProps {
  target: LayoutTemplateTarget | null
  onApply: (tabId: string, root: LayoutShape) => void
  onClose: () => void
}

/**
 * Lays a tab out by a template, tTerm's or one saved from a split tab, and
 * saves the tab's own split as a template.
 */
export function LayoutTemplateDialog({ target, onApply, onClose }: LayoutTemplateDialogProps) {
  const { t } = useTranslation()

  return (
    <Dialog open={target !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle className="flex items-center gap-2">
            <LayoutTemplate size={16} />
            {t("layoutTemplates.title")}
          </DialogTitle>
          <DialogDescription>{t("layoutTemplates.description")}</DialogDescription>
        </DialogHeader>
        {target && <LayoutTemplateBody target={target} onApply={onApply} onClose={onClose} />}
      </DialogContent>
    </Dialog>
  )
}

// Mounted while the dialog is open, so each opening starts with an empty name.
function LayoutTemplateBody({
  target,
  onApply,
  onClose,
}: Omit<LayoutTemplateDialogProps, "target"> & { target: LayoutTemplateTarget }) {
  const { t } = useTranslation()
  const { config } = useConfig()
  const { saveSettings } = useSettingsSave()
  const [name, setName] = React.useState("")
  const saved = config.panes.templates
  const paneCount = getPaneIds(target.root).length

  const saveTemplates = (templates: PaneLayoutTemplate[]) =>
    saveSettings({ panes: { ...config.panes, templates } })

  const apply = (root: LayoutShape) => {
    onApply(target.tabId, root)
    onClose()
  }

  const saveCurrent = async () => {
    if (target.root.type === "pane") return
    const template: PaneLayoutTemplate = {
      id: crypto.randomUUID(),
      name: name.trim() || t("layoutTemplates.untitled", { count: paneCount }),
      root: shapeOfTree(target.root),
    }
    if (await saveTemplates([...saved, template])) setName("")
  }

  const card = (template: PaneLayoutTemplate, label: string, onDelete?: () => void) => {
    const panes = countTemplatePanes(template.root)
    // Without a session to copy, a layout can only rearrange the panes there are.
    const disabled = panes > paneCount && !target.canCopy
    return (
      <div key={template.id} className="layout-template-card-wrap">
        <button
          type="button"
          className="layout-template-card"
          disabled={disabled}
          title={disabled ? t("layoutTemplates.needsSession") : undefined}
          onClick={() => apply(template.root)}
        >
          <div className="layout-template-preview">
            <LayoutShapePreview shape={template.root} />
          </div>
          <span className="layout-template-name">{label}</span>
          <span className="layout-template-count">
            {t("layoutTemplates.paneCount", { count: panes })}
          </span>
        </button>
        {onDelete && (
          <button
            type="button"
            className="layout-template-delete"
            aria-label={t("layoutTemplates.delete", { name: label })}
            onClick={onDelete}
          >
            <Trash2 size={12} />
          </button>
        )}
      </div>
    )
  }

  return (
    <div className="space-y-4">
      <section className="space-y-2">
        <h3 className="text-muted-foreground text-xs font-medium">
          {t("layoutTemplates.builtin")}
        </h3>
        <div className="layout-template-grid">
          {BUILTIN_PANE_TEMPLATES.map((template) =>
            card(template, t(`layoutTemplates.builtins.${template.labelKey}`))
          )}
        </div>
      </section>
      <section className="space-y-2">
        <h3 className="text-muted-foreground text-xs font-medium">{t("layoutTemplates.saved")}</h3>
        {saved.length > 0 ? (
          <div className="layout-template-grid">
            {saved.map((template) =>
              card(
                template,
                template.name,
                () => void saveTemplates(saved.filter(({ id }) => id !== template.id))
              )
            )}
          </div>
        ) : (
          <p className="text-muted-foreground text-xs">{t("layoutTemplates.noneSaved")}</p>
        )}
      </section>
      <section className="space-y-2 border-t pt-4">
        <h3 className="text-muted-foreground text-xs font-medium">
          {t("layoutTemplates.saveCurrent")}
        </h3>
        {target.root.type === "split" ? (
          <div className="flex gap-2">
            <Input
              value={name}
              placeholder={t("layoutTemplates.untitled", { count: paneCount })}
              aria-label={t("layoutTemplates.name")}
              onChange={(event) => setName(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter" && !isImeKeyEvent(event.nativeEvent)) {
                  void saveCurrent()
                }
              }}
            />
            <Button type="button" variant="outline" onClick={() => void saveCurrent()}>
              {t("layoutTemplates.save")}
            </Button>
          </div>
        ) : (
          <p className="text-muted-foreground text-xs">{t("layoutTemplates.splitFirst")}</p>
        )}
      </section>
    </div>
  )
}
