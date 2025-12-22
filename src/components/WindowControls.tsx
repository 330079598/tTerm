import { getCurrentWindow } from '@tauri-apps/api/window';
import { platform } from '@tauri-apps/plugin-os';
import { Minus, Square, X } from 'lucide-react';
import { useEffect, useState } from 'react';

const appWindow = getCurrentWindow();

export function WindowControls() {

  const [os, setOs] = useState<string>('');
  const [isMaximized, setIsMaximized] = useState(false);

  useEffect(() => {
    // detect os
    setOs(platform());

    // listen window resized
    const unlisten = appWindow.onResized(async () => {
      setIsMaximized(await appWindow.isMaximized());
    });

    // init maximized state
    appWindow.isMaximized().then(setIsMaximized);

    return () => {
      unlisten.then(fn => fn());
    };
  }, []);

  const handleMinimize = async () => await appWindow.minimize();
  const handleMaximize = async () => await appWindow.toggleMaximize();
  const handleClose = async () => await appWindow.close();

  // macOS style
  if (os === 'macos') {
    return (
      <div></div>
    );
  }

  // Windows/Linux style
  return (
    <div className="flex h-full ml-auto">
      <button
        onClick={handleMinimize}
        className="px-4 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors flex items-center justify-center"
        title="Minimize"
      >
        <Minus className="w-4 h-4" />
      </button>
      <button
        onClick={handleMaximize}
        className="px-4 hover:bg-gray-200 dark:hover:bg-gray-700 transition-colors flex items-center justify-center"
        title={isMaximized ? 'Restore' : 'Maximize'}
      >
        <Square className="w-3.5 h-3.5" />
      </button>
      <button
        onClick={handleClose}
        className="px-4 hover:bg-red-500 hover:text-white transition-colors flex items-center justify-center"
        title="Close"
      >
        <X className="w-4 h-4" />
      </button>
    </div>
  );
}
