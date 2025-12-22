import { createRootRoute, Link, Outlet } from '@tanstack/react-router'
import { TanStackRouterDevtools } from '@tanstack/react-router-devtools'
import { WindowControls } from '../components/WindowControls'
import { platform } from '@tauri-apps/plugin-os'
import { useEffect, useState } from 'react'

const RootLayout = () => {
    const [os, setOs] = useState<string>('')

    useEffect(() => {
        setOs(platform())
    }, [])

    return (
        <>
            <div className="flex items-center h-8 bg-white dark:bg-gray-900 border-b border-gray-200 dark:border-gray-800 px-3" data-tauri-drag-region>
                <div className={`flex gap-4 px-4 ${os === 'macos' ? 'ml-13' : ''}`}>
                    <Link to="/" className="[&.active]:font-bold">
                        Home
                    </Link>
                    <Link to="/about" className="[&.active]:font-bold">
                        About
                    </Link>
                </div>
                <WindowControls />
            </div>
            <hr />
            <Outlet />
            <TanStackRouterDevtools />
        </>
    )
}

export const Route = createRootRoute({ component: RootLayout })