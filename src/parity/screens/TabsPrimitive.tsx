import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs"
import type { ParityScreenProps } from "@/parity/types"
import { useEffect } from "react"

export function TabsPrimitive({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground h-full w-full p-8">
      <Tabs defaultValue="overview" data-parity="ui.tabs.default">
        <TabsList data-parity="ui.tabs.list">
          <TabsTrigger value="overview" data-parity="ui.tabs.active">
            Overview
          </TabsTrigger>
          <TabsTrigger value="cpu" data-parity="ui.tabs.inactive">
            CPU
          </TabsTrigger>
          <TabsTrigger value="memory" autoFocus data-parity="ui.tabs.focus">
            Memory
          </TabsTrigger>
        </TabsList>
        <TabsContent value="overview" data-parity="ui.tabs.content">
          Active overview content
        </TabsContent>
      </Tabs>
    </main>
  )
}

/** The className combination used by TerminalTab/ServerMonitorPanel.tsx. */
export function TabsServerMonitor({ onReady }: ParityScreenProps) {
  useEffect(() => onReady(), [onReady])
  return (
    <main className="bg-background text-foreground h-full w-full p-8">
      <Tabs
        defaultValue="overview"
        className="server-monitor-panel-tabs"
        data-parity="ui.tabs.server"
      >
        <header className="server-monitor-panel-header">
          <TabsList className="server-monitor-panel-tab-list" data-parity="ui.tabs.server-list">
            <TabsTrigger
              value="overview"
              className="server-monitor-panel-tab"
              data-parity="ui.tabs.server-active"
            >
              Overview
            </TabsTrigger>
            <TabsTrigger
              value="cpu"
              className="server-monitor-panel-tab"
              data-parity="ui.tabs.server-inactive"
            >
              CPU
            </TabsTrigger>
            <TabsTrigger
              value="memory"
              className="server-monitor-panel-tab"
              data-parity="ui.tabs.server-focus"
              autoFocus
            >
              Memory
            </TabsTrigger>
          </TabsList>
        </header>
        <TabsContent
          value="overview"
          className="server-monitor-panel-content"
          data-parity="ui.tabs.server-content"
        >
          Active overview content
        </TabsContent>
      </Tabs>
    </main>
  )
}
