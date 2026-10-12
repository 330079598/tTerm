import { useTranslation } from "react-i18next"
import { Archive, Cloud, History, Upload } from "lucide-react"

import type { MigrationView } from "@/components/SettingsDialog/dataMigration"
import { Button } from "@/components/ui/button"

interface MigrationViewTabsProps {
  activeView: MigrationView
  showView: (view: MigrationView) => void
}

/** Switches between backing up, WebDAV, importing and the backup history. */
export function MigrationViewTabs({ activeView, showView }: MigrationViewTabsProps) {
  const { t } = useTranslation()

  return (
    <div
      className="bg-muted/40 grid grid-cols-4 gap-1 rounded-md p-1"
      role="group"
      aria-label={t("dataMigration.title")}
    >
      <Button
        type="button"
        size="sm"
        variant={activeView === "backup" ? "default" : "ghost"}
        aria-pressed={activeView === "backup"}
        onClick={() => showView("backup")}
      >
        <Archive size={14} />
        {t("dataMigration.backupTab", { defaultValue: "Backup" })}
      </Button>
      <Button
        type="button"
        size="sm"
        variant={activeView === "webdav" ? "default" : "ghost"}
        aria-pressed={activeView === "webdav"}
        onClick={() => showView("webdav")}
      >
        <Cloud size={14} />
        {t("dataMigration.webdavTab")}
      </Button>
      <Button
        type="button"
        size="sm"
        variant={activeView === "import" ? "default" : "ghost"}
        aria-pressed={activeView === "import"}
        onClick={() => showView("import")}
      >
        <Upload size={14} />
        {t("dataMigration.importTab", { defaultValue: "Import" })}
      </Button>
      <Button
        type="button"
        size="sm"
        variant={activeView === "history" ? "default" : "ghost"}
        aria-pressed={activeView === "history"}
        onClick={() => showView("history")}
      >
        <History size={14} />
        {t("dataMigration.historyTab", { defaultValue: "History" })}
      </Button>
    </div>
  )
}
