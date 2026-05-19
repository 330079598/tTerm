import React from "react"
import ReactMarkdown from "react-markdown"
import remarkEmoji from "remark-emoji"
import remarkGfm from "remark-gfm"

function isSafeMarkdownHref(href: string | undefined) {
  if (!href) return false
  const trimmed = href.trim().toLowerCase()
  return (
    trimmed.startsWith("https://") || trimmed.startsWith("http://") || trimmed.startsWith("mailto:")
  )
}

const releaseNotesEmojiFallbacks: Record<string, string> = {
  technologist: "\u{1F9D1}\u200D\u{1F4BB}",
}

function normalizeReleaseNotesEmojiFallbacks(notes: string) {
  return notes.replace(/:([\w+-]+):/g, (match, shortcode: string) => {
    return releaseNotesEmojiFallbacks[shortcode] ?? match
  })
}

interface UpdateReleaseNotesProps {
  notes: string
}

export function UpdateReleaseNotes({ notes }: UpdateReleaseNotesProps) {
  const renderedReleaseNotes = React.useMemo(
    () => normalizeReleaseNotesEmojiFallbacks(notes),
    [notes]
  )

  return (
    <ReactMarkdown
      remarkPlugins={[remarkGfm, remarkEmoji]}
      skipHtml
      components={{
        a: ({ href, children }) =>
          isSafeMarkdownHref(href) ? (
            <a
              className="text-primary underline underline-offset-2"
              href={href}
              rel="noreferrer"
              target="_blank"
            >
              {children}
            </a>
          ) : (
            <>{children}</>
          ),
        blockquote: ({ children }) => (
          <blockquote className="my-2 border-l-2 pl-3 italic">{children}</blockquote>
        ),
        code: ({ children }) => (
          <code className="bg-muted rounded px-1 py-0.5 font-mono">{children}</code>
        ),
        h1: ({ children }) => <h4 className="text-foreground mb-2 font-semibold">{children}</h4>,
        h2: ({ children }) => <h4 className="text-foreground mb-2 font-semibold">{children}</h4>,
        h3: ({ children }) => <h5 className="text-foreground mb-2 font-semibold">{children}</h5>,
        li: ({ children }) => <li className="my-1">{children}</li>,
        ol: ({ children }) => <ol className="my-2 list-decimal pl-4">{children}</ol>,
        p: ({ children }) => <p className="my-2 first:mt-0 last:mb-0">{children}</p>,
        pre: ({ children }) => (
          <pre className="bg-background my-2 overflow-auto rounded border p-2">{children}</pre>
        ),
        ul: ({ children }) => <ul className="my-2 list-disc pl-4">{children}</ul>,
      }}
    >
      {renderedReleaseNotes}
    </ReactMarkdown>
  )
}
