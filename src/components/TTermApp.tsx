import React, { useState, useCallback } from 'react';
import { platform } from '@tauri-apps/plugin-os';
import { useEffect } from 'react';
import { 
  Plus, 
  X, 
  Terminal, 
  Server, 
  FolderOpen, 
  Zap,
  Settings
} from 'lucide-react';
import { ContextMenu } from './ContextMenu';
import { WindowControls } from './WindowControls';
import { ConnectionDialog } from './ConnectionDialog';
import { useTabs } from '../hooks/useTabs';
import { useSessionPersistence } from '../hooks/useSessionPersistence';
import { useConnectionManager } from '../hooks/useConnectionManager';
import { Tab, TabContextMenuAction } from '../types/tab';

const getTabIcon = (type: Tab['type']) => {
  switch (type) {
    case 'terminal':
      return <Terminal className="tab-icon" />;
    case 'ssh':
      return <Server className="tab-icon" />;
    case 'sftp':
      return <FolderOpen className="tab-icon" />;
    case 'serial':
      return <Zap className="tab-icon" />;
    default:
      return <Terminal className="tab-icon" />;
  }
};

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
  const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
  const [dropIndicatorIndex, setDropIndicatorIndex] = useState<number | null>(null);
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
    updateTab,
    moveTab,
    duplicateTab,
    closeOtherTabs,
    closeTabsToRight,
    restoreSession,
  } = useTabs();

  const { saveSession, loadSession, clearSession } = useSessionPersistence();
  const { 
    getConnectionState, 
    cleanupConnection 
  } = useConnectionManager();

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

  // Drag and drop handlers
  const handleDragStart = useCallback((e: React.DragEvent, index: number) => {
    console.log('Drag start:', index);
    setDraggedIndex(index);
    e.dataTransfer.effectAllowed = 'move';
    e.dataTransfer.setData('text/plain', index.toString());
  }, []);

  const handleDragOver = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    e.stopPropagation();
    e.dataTransfer.dropEffect = 'move';
    console.log('Drag over - this should print frequently');
  }, []);

  const handleDrop = useCallback((e: React.DragEvent, dropIndex: number) => {
    e.preventDefault();
    e.stopPropagation();
    
    console.log('Drop event:', { draggedIndex, dropIndex });
    
    if (draggedIndex !== null && draggedIndex !== dropIndex) {
      console.log('Moving tab from', draggedIndex, 'to', dropIndex);
      moveTab(draggedIndex, dropIndex);
    }
    
    setDraggedIndex(null);
    setDropIndicatorIndex(null);
  }, [draggedIndex, moveTab]);

  const handleDragEnter = useCallback((e: React.DragEvent, index: number) => {
    e.preventDefault();
    e.stopPropagation();
    console.log('Drag enter:', index, 'draggedIndex:', draggedIndex);
    if (draggedIndex !== null && draggedIndex !== index) {
      setDropIndicatorIndex(index);
    }
  }, [draggedIndex]);

  const handleDragLeave = useCallback((e: React.DragEvent) => {
    e.preventDefault();
    console.log('Drag leave');
    // Delay clearing indicator to avoid flicker when quickly entering/leaving
    setTimeout(() => {
      setDropIndicatorIndex(null);
    }, 50);
  }, []);

  const handleDragEnd = useCallback(() => {
    console.log('Drag end');
    setDraggedIndex(null);
    setDropIndicatorIndex(null);
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
            <div className="tab-list">
              {tabs.map((tab, index) => (
                <React.Fragment key={tab.id}>
                  {dropIndicatorIndex === index && (
                    <div className="tab-drop-indicator" />
                  )}
                  <div
                    className={`tab ${tab.id === activeTabId ? 'active' : ''} ${tab.isModified ? 'modified' : ''} ${draggedIndex === index ? 'dragging' : ''}`}
                    onClick={() => setActiveTab(tab.id)}
                    onContextMenu={(e) => {
                      const actions: TabContextMenuAction[] = [
                        { label: 'New Tab', action: 'new', icon: 'plus' },
                        { label: 'Duplicate Tab', action: 'duplicate', icon: 'copy' },
                        { separator: true, label: '', action: '' },
                        { label: 'Close Tab', action: 'close', icon: 'x' },
                        { label: 'Close Other Tabs', action: 'close-others' },
                        { label: 'Close Tabs to the Right', action: 'close-right' },
                      ];
                      handleTabContextMenu(e, tab, actions);
                    }}
                    draggable
                    onDragStart={(e) => handleDragStart(e, index)}
                    onDragEnd={handleDragEnd}
                    onDragOver={handleDragOver}
                    onDragEnter={(e) => handleDragEnter(e, index)}
                    onDragLeave={handleDragLeave}
                    onDrop={(e) => handleDrop(e, index)}
                    title={`${tab.title}${tab.connection ? ` (${tab.connection.host})` : ''}`}
                  >
                    {getTabIcon(tab.type)}
                    <span className="tab-title">{tab.title}</span>
                    <button
                      className="tab-close"
                      onClick={(e) => {
                        e.stopPropagation();
                        handleRemoveTab(tab.id);
                      }}
                      title="Close tab"
                    >
                      <X size={12} />
                    </button>
                  </div>
                </React.Fragment>
              ))}
            </div>
            
            {/* Add tab button - right after tabs */}
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
    </div>
  );
};