import "@/components/LayoutTemplateDialog/LayoutShapePreview.css"
import type { LayoutShape } from "@/lib/paneTemplates"

/** A small drawing of a template's panes, each split sized as the template sizes it. */
export function LayoutShapePreview({ shape }: { shape: LayoutShape }) {
  if (shape.type === "pane") {
    return <div className="layout-preview-pane" />
  }
  return (
    <div className={`layout-preview-split ${shape.orientation}`}>
      {shape.children.map((child, index) => (
        <div key={index} className="layout-preview-cell" style={{ flexGrow: shape.sizes[index] }}>
          <LayoutShapePreview shape={child} />
        </div>
      ))}
    </div>
  )
}
