import React from "react"

export const ThemeEditor = React.lazy(() =>
  import("@/components/ThemeEditor").then((module) => ({
    default: module.ThemeEditor,
  }))
)

export const ThemeGallery = React.lazy(() =>
  import("@/components/ThemeGallery").then((module) => ({
    default: module.ThemeGallery,
  }))
)

export const ThemeImportDialog = React.lazy(() =>
  import("@/components/ThemeImportDialog").then((module) => ({
    default: module.ThemeImportDialog,
  }))
)
