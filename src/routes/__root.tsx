import {createRootRoute} from '@tanstack/react-router'
import { lazy } from 'react'
import {TTermApp} from '../components/TTermApp'

const TanStackRouterDevtools = import.meta.env.DEV
  ? lazy(() =>
      import('@tanstack/react-router-devtools').then((m) => ({
        default: m.TanStackRouterDevtools,
      }))
    )
  : () => null;

const RootLayout = () => {
    return (
        <>
            <TTermApp/>
            <TanStackRouterDevtools />
        </>
    )
}

export const Route = createRootRoute({ component: RootLayout })