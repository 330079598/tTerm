import "@/App.css"
import { RouterProvider, createRouter } from "@tanstack/react-router"
import { ErrorBoundary } from "@/components/ErrorBoundary"
import { Toaster } from "@/components/ui/toaster"

// Import the generated route tree
import { routeTree } from "@/routeTree.gen"

// Create a new router instance
const router = createRouter({ routeTree })

// Register the router instance for type safety
declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}

function App() {
  return (
    <>
      <ErrorBoundary scope="app">
        <RouterProvider router={router} />
      </ErrorBoundary>
      <Toaster />
    </>
  )
}

export default App
