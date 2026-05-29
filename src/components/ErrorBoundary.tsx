import React from "react"
import { useTranslation } from "react-i18next"

import { Button } from "@/components/ui/button"

type ErrorBoundaryProps = {
  children: React.ReactNode
  resetKey?: string | number | null
  scope?: string
}

type ErrorBoundaryState = {
  error: Error | null
}

class ErrorBoundaryInner extends React.Component<
  ErrorBoundaryProps & { title: string; description: string; resetLabel: string },
  ErrorBoundaryState
> {
  state: ErrorBoundaryState = { error: null }

  static getDerivedStateFromError(error: Error): ErrorBoundaryState {
    return { error }
  }

  componentDidCatch(error: Error, errorInfo: React.ErrorInfo) {
    console.error(`Error boundary caught an error in ${this.props.scope ?? "app"}`, {
      error,
      errorInfo,
    })
  }

  componentDidUpdate(previousProps: ErrorBoundaryProps) {
    if (previousProps.resetKey !== this.props.resetKey && this.state.error) {
      this.setState({ error: null })
    }
  }

  render() {
    if (!this.state.error) {
      return this.props.children
    }

    return (
      <div className="flex h-full w-full items-center justify-center bg-background p-6 text-foreground">
        <div className="max-w-md space-y-3 rounded-md border border-border bg-card p-4 text-card-foreground shadow-sm">
          <div className="space-y-1">
            <h2 className="text-sm font-semibold">{this.props.title}</h2>
            <p className="text-sm text-muted-foreground">{this.props.description}</p>
          </div>
          <pre className="max-h-28 overflow-auto rounded bg-muted p-2 text-xs text-muted-foreground">
            {this.state.error.message}
          </pre>
          <Button size="sm" onClick={() => this.setState({ error: null })}>
            {this.props.resetLabel}
          </Button>
        </div>
      </div>
    )
  }
}

export function ErrorBoundary({ children, resetKey, scope }: ErrorBoundaryProps) {
  const { t } = useTranslation()

  return (
    <ErrorBoundaryInner
      resetKey={resetKey}
      scope={scope}
      title={t("errors.boundaryTitle", { defaultValue: "Something went wrong" })}
      description={t("errors.boundaryDescription", {
        defaultValue: "This view crashed. You can retry without restarting the app.",
      })}
      resetLabel={t("errors.retry", { defaultValue: "Retry" })}
    >
      {children}
    </ErrorBoundaryInner>
  )
}
