import { history, historyKeymap, indentWithTab } from "@codemirror/commands"
import { css } from "@codemirror/lang-css"
import { html } from "@codemirror/lang-html"
import { javascript } from "@codemirror/lang-javascript"
import { json } from "@codemirror/lang-json"
import { markdown } from "@codemirror/lang-markdown"
import { rust } from "@codemirror/lang-rust"
import { yaml } from "@codemirror/lang-yaml"
import {
  bracketMatching,
  foldGutter,
  foldKeymap,
  HighlightStyle,
  indentOnInput,
  syntaxHighlighting,
} from "@codemirror/language"
import { searchKeymap } from "@codemirror/search"
import { Compartment, EditorState, type Extension } from "@codemirror/state"
import {
  drawSelection,
  EditorView,
  highlightActiveLine,
  highlightActiveLineGutter,
  keymap,
  lineNumbers,
} from "@codemirror/view"
import { tags } from "@lezer/highlight"
import React, { useEffect, useMemo, useRef } from "react"

interface CodeMirrorEditorProps {
  className?: string
  fileName?: string
  onChange: (value: string) => void
  value: string
}

const languageCompartment = new Compartment()
const themeCompartment = new Compartment()

const ttermHighlightStyle = HighlightStyle.define([
  { tag: tags.keyword, color: "hsl(var(--primary))" },
  { tag: [tags.atom, tags.bool, tags.null, tags.number], color: "hsl(28 85% 55%)" },
  { tag: [tags.string, tags.special(tags.string)], color: "hsl(145 55% 45%)" },
  { tag: tags.regexp, color: "hsl(325 72% 60%)" },
  {
    tag: [tags.function(tags.propertyName), tags.function(tags.variableName), tags.labelName],
    color: "hsl(190 80% 45%)",
  },
  {
    tag: [tags.className, tags.definition(tags.typeName), tags.typeName],
    color: "hsl(45 85% 50%)",
  },
  { tag: [tags.propertyName, tags.variableName], color: "hsl(var(--foreground))" },
  {
    tag: [tags.comment, tags.lineComment, tags.blockComment],
    color: "hsl(var(--muted-foreground))",
  },
  { tag: tags.meta, color: "hsl(var(--muted-foreground))" },
  { tag: tags.invalid, color: "hsl(var(--destructive))" },
])

function extensionForFileName(fileName?: string): Extension {
  const extension = fileName?.toLowerCase().split(".").pop()

  switch (extension) {
    case "cjs":
    case "js":
    case "jsx":
    case "mjs":
      return javascript({ jsx: true })
    case "ts":
      return javascript({ typescript: true })
    case "tsx":
      return javascript({ jsx: true, typescript: true })
    case "json":
    case "jsonc":
      return json()
    case "rs":
      return rust()
    case "md":
    case "markdown":
      return markdown()
    case "yaml":
    case "yml":
      return yaml()
    case "html":
    case "htm":
    case "xml":
      return html()
    case "css":
    case "scss":
    case "sass":
    case "less":
      return css()
    default:
      return []
  }
}

function editorTheme(): Extension {
  return EditorView.theme({
    "&": {
      backgroundColor: "hsl(var(--background))",
      color: "hsl(var(--foreground))",
      fontFamily:
        'ui-monospace, SFMono-Regular, "SF Mono", Consolas, "Liberation Mono", Menlo, monospace',
      fontSize: "13px",
      height: "100%",
    },
    ".cm-activeLine": {
      backgroundColor: "hsl(var(--muted) / 0.45)",
    },
    ".cm-activeLineGutter": {
      backgroundColor: "hsl(var(--muted) / 0.65)",
      color: "hsl(var(--foreground))",
    },
    ".cm-content": {
      caretColor: "hsl(var(--foreground))",
      minHeight: "100%",
      padding: "12px 0",
    },
    ".cm-cursor": {
      borderLeftColor: "hsl(var(--foreground))",
    },
    ".cm-focused": {
      outline: "none",
    },
    ".cm-gutters": {
      backgroundColor: "hsl(var(--muted) / 0.35)",
      borderRight: "1px solid hsl(var(--border))",
      color: "hsl(var(--muted-foreground))",
    },
    ".cm-line": {
      lineHeight: "1.55",
      padding: "0 12px",
    },
    ".cm-scroller": {
      overflow: "auto",
    },
    ".cm-selectionBackground, .cm-content ::selection": {
      backgroundColor: "hsl(var(--primary) / 0.24) !important",
    },
  })
}

function codeMirrorCspNonceExtension(): Extension {
  const nonce =
    Array.from(document.getElementsByTagName("style"))
      .map((style) => style.nonce)
      .find(Boolean) ?? ""

  return nonce ? EditorView.cspNonce.of(nonce) : []
}

export const CodeMirrorEditor: React.FC<CodeMirrorEditorProps> = ({
  className,
  fileName,
  onChange,
  value,
}) => {
  const containerRef = useRef<HTMLDivElement | null>(null)
  const viewRef = useRef<EditorView | null>(null)
  const initialFileNameRef = useRef(fileName)
  const initialValueRef = useRef(value)
  const onChangeRef = useRef(onChange)
  const languageExtension = useMemo(() => extensionForFileName(fileName), [fileName])

  useEffect(() => {
    onChangeRef.current = onChange
  }, [onChange])

  useEffect(() => {
    if (!containerRef.current) {
      return
    }

    const initialLanguageExtension = extensionForFileName(initialFileNameRef.current)
    const state = EditorState.create({
      doc: initialValueRef.current,
      extensions: [
        codeMirrorCspNonceExtension(),
        lineNumbers(),
        foldGutter(),
        highlightActiveLineGutter(),
        history(),
        drawSelection(),
        indentOnInput(),
        bracketMatching(),
        syntaxHighlighting(ttermHighlightStyle, { fallback: true }),
        highlightActiveLine(),
        keymap.of([indentWithTab, ...historyKeymap, ...foldKeymap, ...searchKeymap]),
        languageCompartment.of(initialLanguageExtension),
        themeCompartment.of(editorTheme()),
        EditorView.lineWrapping,
        EditorView.updateListener.of((update) => {
          if (update.docChanged) {
            onChangeRef.current(update.state.doc.toString())
          }
        }),
      ],
    })

    const view = new EditorView({
      parent: containerRef.current,
      state,
    })
    viewRef.current = view

    return () => {
      view.destroy()
      viewRef.current = null
    }
  }, [])

  useEffect(() => {
    const view = viewRef.current
    if (!view) {
      return
    }

    const currentValue = view.state.doc.toString()
    if (currentValue === value) {
      return
    }

    view.dispatch({
      changes: {
        from: 0,
        insert: value,
        to: view.state.doc.length,
      },
    })
  }, [value])

  useEffect(() => {
    const view = viewRef.current
    if (!view) {
      return
    }

    view.dispatch({
      effects: languageCompartment.reconfigure(languageExtension),
    })
  }, [languageExtension])

  return <div ref={containerRef} className={className} />
}
