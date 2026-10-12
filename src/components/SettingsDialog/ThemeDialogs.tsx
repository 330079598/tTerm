import React from "react"

import {
  ThemeEditor,
  ThemeGallery,
  ThemeImportDialog,
} from "@/components/SettingsDialog/settingsLazy"
import type { useThemeSettingsHandlers } from "@/components/SettingsDialog/useThemeSettingsHandlers"

interface ThemeDialogsProps {
  editingThemeId: string | null
  setEditingThemeId: (id: string | null) => void
  creatingFromTheme: string | null
  setCreatingFromTheme: (id: string | null) => void
  themeGalleryOpen: boolean
  setThemeGalleryOpen: (open: boolean) => void
  themeImportOpen: boolean
  setThemeImportOpen: (open: boolean) => void
  themeHandlers: ReturnType<typeof useThemeSettingsHandlers>
}

/** The theme editor, gallery and import dialogs the appearance page opens, loaded on demand. */
export const ThemeDialogs: React.FC<ThemeDialogsProps> = ({
  editingThemeId,
  setEditingThemeId,
  creatingFromTheme,
  setCreatingFromTheme,
  themeGalleryOpen,
  setThemeGalleryOpen,
  themeImportOpen,
  setThemeImportOpen,
  themeHandlers,
}) => (
  <>
    {editingThemeId && (
      <React.Suspense fallback={null}>
        <ThemeEditor themeId={editingThemeId} onClose={() => setEditingThemeId(null)} />
      </React.Suspense>
    )}

    {creatingFromTheme && (
      <React.Suspense fallback={null}>
        <ThemeEditor baseThemeId={creatingFromTheme} onClose={() => setCreatingFromTheme(null)} />
      </React.Suspense>
    )}

    {themeGalleryOpen && (
      <React.Suspense fallback={null}>
        <ThemeGallery
          onApply={themeHandlers.handleGalleryApply}
          onCopy={themeHandlers.handleCopyCatalogTheme}
          onClose={() => setThemeGalleryOpen(false)}
        />
      </React.Suspense>
    )}

    {themeImportOpen && (
      <React.Suspense fallback={null}>
        <ThemeImportDialog
          onImported={themeHandlers.handleThemesImported}
          onClose={() => setThemeImportOpen(false)}
        />
      </React.Suspense>
    )}
  </>
)
