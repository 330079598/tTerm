import {createRootRoute} from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import {TTermApp} from '../components/TTermApp'

const RootLayout = () => {
    return (
        <>
            <TTermApp/>
            <TanStackRouterDevtools />
        </>
    )
}

export const Route = createRootRoute({ component: RootLayout })