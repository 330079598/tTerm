import React, { useState, useCallback } from 'react';
import { platform } from '@tauri-apps/plugin-os';
import { useEffect } from 'react';
import { Plus, Settings } from 'lucide-react';
import { ContextMenu } from './ContextMenu';
import { WindowControls } from './WindowControls';
import { ConnectionDialog } from './ConnectionDialog';
import { ThemeSwitcher } from './ThemeSwitcher';
import { TabBar } from './TabBar';
import { useTabs } from '../hooks/useTabs';
import { useSessionPersistence } from '../hooks/useSessionPersistence';
import { useConnectionManager } from '../hooks/useConnectionManager';
import { Tab, TabContextMenuAction } from '../types/tab';

interface ContextMenuState {
  visible: boolean;
  x: number;
  y: number;
  tab: Tab | null;
  actions: TabContextMenuAction[];
}

export const TTermApp: React.FC = () => {
  const [os, setOs] = useState<string>('');
  const [showConnectionDialog, setShowConnectionDialog] = useState(false);
  const [showThemeSwitcher, setShowThemeSwitcher] = useState(false);
  const [contextMenu, setContextMenu] = useState<ContextMenuState>({
    visible: false,
    x: 0,
    y: 0,
    tab: null,
    actions: []
  });

  const {
    tabs,
    activeTabId,
    addTab,
    removeTab,
    setActiveTab,
    moveTab,
    duplicateTab,
    closeOtherTabs,
    closeTabsToRight,
    restoreSession,
  } = useTabs();

  const { saveSession, loadSession, clearSession } = useSessionPersistence();
  const { cleanupConnection } = useConnectionManager();

  useEffect(() => {
    setOs(platform());
    
    // Try to restore previous session
    const savedSession = loadSession();
    if (savedSession && savedSession.tabs.length > 0) {
      console.log('Restoring session with', savedSession.tabs.length, 'tabs');
      restoreSession(savedSession.tabs, savedSession.activeTabId);
    } else {
      // If no saved session, create default tab
      console.log('No saved session, creating default tab');
      addTab({
        title: 'Terminal',
        type: 'terminal',
        isModified: false,
      });
    }
  }, [addTab, loadSession, restoreSession]);

  // Save session state when tabs or active tab changes
  useEffect(() => {
    if (tabs.length > 0) {
      saveSession(tabs, activeTabId);
    }
  }, [tabs, activeTabId, saveSession]);

  const handleNewTab = useCallback(() => {
    setShowConnectionDialog(true);
  }, []);

  const handleConnect = useCallback((connection: Omit<Tab, 'id' | 'isActive'>) => {
    addTab(connection);
  }, [addTab]);

  const handleRemoveTab = useCallback((id: string) => {
    cleanupConnection(id);
    removeTab(id);
  }, [removeTab, cleanupConnection]);

  const handleTabContextMenu = useCallback((e: React.MouseEvent, tab: Tab, actions: TabContextMenuAction[]) => {
    e.preventDefault();
    setContextMenu({
      visible: true,
      x: e.clientX,
      y: e.clientY,
      tab,
      actions
    });
  }, []);

  const handleContextMenuAction = useCallback((action: string) => {
    if (!contextMenu.tab) {
      // Handle settings menu actions
      switch (action) {
        case 'change-theme':
          setShowThemeSwitcher(true);
          break;
        case 'clear-session':
          clearSession();
          // Reload page to reset state
          window.location.reload();
          break;
        case 'about':
          alert('TTerm - Modern Terminal Emulator\nVersion 0.1.0\nBuilt with Tauri + React');
          break;
        default:
          break;
      }
      return;
    }

    const tab = contextMenu.tab;

    switch (action) {
      case 'new':
        handleNewTab();
        break;
      case 'duplicate':
        duplicateTab(tab.id);
        break;
      case 'close':
        handleRemoveTab(tab.id);
        break;
      case 'close-others':
        closeOtherTabs(tab.id);
        break;
      case 'close-right':
        closeTabsToRight(tab.id);
        break;
      default:
        break;
    }
  }, [contextMenu.tab, handleNewTab, duplicateTab, handleRemoveTab, closeOtherTabs, closeTabsToRight, clearSession]);

  const handleCloseContextMenu = useCallback(() => {
    setContextMenu(prev => ({ ...prev, visible: false }));
  }, []);

  // Handle settings button click
  const handleSettingsClick = useCallback(() => {
    const actions: TabContextMenuAction[] = [
      { label: 'Change Theme', action: 'change-theme', icon: 'palette' },
      { separator: true, label: '', action: '' },
      { label: 'Clear Session Data', action: 'clear-session', icon: 'x' },
      { separator: true, label: '', action: '' },
      { label: 'About TTerm', action: 'about' },
    ];
    
    // Show context menu near settings button
    setContextMenu({
      visible: true,
      x: window.innerWidth - 200, // Near settings button
      y: 50,
      tab: null,
      actions
    });
  }, []);

  const renderTabContent = () => {
    if (!activeTabId) {
      return (
        <div className="terminal-placeholder">
          <h3>Welcome to TTerm</h3>
          <p>
            A modern terminal emulator inspired by Tabby. 
            Click the + button to create a new connection.
          </p>
          <button 
            onClick={handleNewTab}
            className="btn-primary"
            style={{ marginTop: '16px' }}
          >
            New Connection
          </button>
        </div>
      );
    }

    const activeTab = tabs.find(t => t.id === activeTabId);
    if (!activeTab) return null;

    // This is where tab content will be rendered
    // You can render different content based on activeTab.type
    // e.g., terminal emulator, SSH client, SFTP browser, etc.
    return (
      <div className="tab-content-container">
        {/* Future terminal/SSH/SFTP content will be rendered here */}
        <div className="tab-content-placeholder">
          <div className="tab-info">
            <h3>{activeTab.title}</h3>
            <p>Type: {activeTab.type.toUpperCase()}</p>
            {activeTab.connection && (
              <p>
                {activeTab.connection.host}:{activeTab.connection.port}
                {activeTab.connection.username && ` (${activeTab.connection.username})`}
              </p>
            )}
          </div>
          <div className="tab-hint">
            <p>This is where the {activeTab.type} interface will be rendered.</p>
            <p>You can implement your own terminal/SSH/SFTP component here.</p>
          </div>
        </div>
      </div>
    );
  };

  return (
    <div className={`app ${os === 'macos' ? 'macos' : ''}`}>
      {/* Combined Title Bar and Tab Bar */}
      <div className="title-bar" data-tauri-drag-region>
        <div className="title-bar-left">
          {/* Tab Bar integrated into title bar */}
          <div className="tab-list-container">
            <TabBar
              tabs={tabs}
              activeTabId={activeTabId}
              onTabClick={setActiveTab}
              onTabClose={handleRemoveTab}
              onNewTab={handleNewTab}
              onTabMove={moveTab}
              onContextMenu={handleTabContextMenu}
            />
            <div className="tab-add-button">
              <button
                className="tab-action"
                onClick={handleNewTab}
                title="New tab"
              >
                <Plus size={16} />
              </button>
            </div>
          </div>
        </div>
        
        {/* Draggable space (10% of window width) */}
        <div className="drag-space" data-tauri-drag-region></div>
        
        {/* Settings and window controls */}
        <div className="title-bar-right">
          <button
            className="tab-action settings-button"
            onClick={handleSettingsClick}
            title="Settings"
          >
            <Settings size={16} />
          </button>
          <WindowControls />
        </div>
      </div>

      {/* Content Area */}
      <div className="content-area">
        {renderTabContent()}
      </div>

      {/* Connection Dialog */}
      <ConnectionDialog
        isOpen={showConnectionDialog}
        onClose={() => setShowConnectionDialog(false)}
        onConnect={handleConnect}
      />

      {/* Context Menu */}
      {contextMenu.visible && (
        <ContextMenu
          x={contextMenu.x}
          y={contextMenu.y}
          actions={contextMenu.actions}
          onAction={handleContextMenuAction}
          onClose={handleCloseContextMenu}
        />
      )}

      {/* Theme Switcher */}
      {showThemeSwitcher && (
        <ThemeSwitcher onClose={() => setShowThemeSwitcher(false)} />
      )}
    </div>
  );
};