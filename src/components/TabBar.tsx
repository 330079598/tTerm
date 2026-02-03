import React, {useCallback, useState} from 'react';
import {FolderOpen, MoreHorizontal, Plus, Server, Terminal, X, Zap,} from 'lucide-react';
import {Tab, TabContextMenuAction} from '../types/tab';

interface TabBarProps {
    tabs: Tab[];
    activeTabId: string | null;
    onTabClick: (id: string) => void;
    onTabClose: (id: string) => void;
    onNewTab: () => void;
    onTabMove: (fromIndex: number, toIndex: number) => void;
    onContextMenu: (tab: Tab, actions: TabContextMenuAction[]) => void;
}

interface TabItemProps {
    tab: Tab;
    isActive: boolean;
    onTabClick: (id: string) => void;
    onTabClose: (id: string) => void;
    onContextMenu: (tab: Tab, actions: TabContextMenuAction[]) => void;
    onDragStart: (e: React.DragEvent, index: number) => void;
    onDragOver: (e: React.DragEvent) => void;
    onDrop: (e: React.DragEvent, index: number) => void;
    index: number;
}

const getTabIcon = (type: Tab['type']) => {
    switch (type) {
        case 'terminal':
            return <Terminal className="tab-icon"/>;
        case 'ssh':
            return <Server className="tab-icon"/>;
        case 'sftp':
            return <FolderOpen className="tab-icon"/>;
        case 'serial':
            return <Zap className="tab-icon"/>;
        default:
            return <Terminal className="tab-icon"/>;
    }
};

const TabItem: React.FC<TabItemProps> = ({
                                             tab,
                                             isActive,
                                             onTabClick,
                                             onTabClose,
                                             onContextMenu,
                                             onDragStart,
                                             onDragOver,
                                             onDrop,
                                             index
                                         }) => {
    const [isDragging, setIsDragging] = useState(false);

    const handleContextMenu = useCallback((e: React.MouseEvent) => {
        e.preventDefault();
        const actions: TabContextMenuAction[] = [
            {label: 'New Tab', action: 'new', icon: 'plus'},
            {label: 'Duplicate Tab', action: 'duplicate', icon: 'copy'},
            {separator: true, label: '', action: ''},
            {label: 'Close Tab', action: 'close', icon: 'x'},
            {label: 'Close Other Tabs', action: 'close-others'},
            {label: 'Close Tabs to the Right', action: 'close-right'},
        ];
        onContextMenu(tab, actions);
    }, [tab, onContextMenu]);

    const handleDragStart = useCallback((e: React.DragEvent) => {
        setIsDragging(true);
        onDragStart(e, index);
    }, [onDragStart, index]);

    const handleDragEnd = useCallback(() => {
        setIsDragging(false);
    }, []);

    return (
        <div
            className={`tab ${isActive ? 'active' : ''} ${isDragging ? 'dragging' : ''} ${tab.isModified ? 'modified' : ''}`}
            onClick={() => onTabClick(tab.id)}
            onContextMenu={handleContextMenu}
            draggable
            onDragStart={handleDragStart}
            onDragEnd={handleDragEnd}
            onDragOver={onDragOver}
            onDrop={(e) => onDrop(e, index)}
            title={`${tab.title}${tab.connection ? ` (${tab.connection.host})` : ''}`}
        >
            {getTabIcon(tab.type)}
            <span className="tab-title">{tab.title}</span>
            <button
                className="tab-close"
                onClick={(e) => {
                    e.stopPropagation();
                    onTabClose(tab.id);
                }}
                title="Close tab"
            >
                <X size={12}/>
            </button>
        </div>
    );
};

export const TabBar: React.FC<TabBarProps> = ({
                                                  tabs,
                                                  activeTabId,
                                                  onTabClick,
                                                  onTabClose,
                                                  onNewTab,
                                                  onTabMove,
                                                  onContextMenu
                                              }) => {
    const [draggedIndex, setDraggedIndex] = useState<number | null>(null);
    const [dropIndicatorIndex, setDropIndicatorIndex] = useState<number | null>(null);

    const handleDragStart = useCallback((e: React.DragEvent, index: number) => {
        setDraggedIndex(index);
        e.dataTransfer.effectAllowed = 'move';
        e.dataTransfer.setData('text/plain', index.toString());
    }, []);

    const handleDragOver = useCallback((e: React.DragEvent) => {
        e.preventDefault();
        e.dataTransfer.dropEffect = 'move';
    }, []);

    const handleDrop = useCallback((e: React.DragEvent, dropIndex: number) => {
        e.preventDefault();

        if (draggedIndex !== null && draggedIndex !== dropIndex) {
            onTabMove(draggedIndex, dropIndex);
        }

        setDraggedIndex(null);
        setDropIndicatorIndex(null);
    }, [draggedIndex, onTabMove]);

    const handleDragEnter = useCallback((e: React.DragEvent, index: number) => {
        e.preventDefault();
        if (draggedIndex !== null && draggedIndex !== index) {
            setDropIndicatorIndex(index);
        }
    }, [draggedIndex]);

    const handleDragLeave = useCallback(() => {
        setDropIndicatorIndex(null);
    }, []);

    return (
        <div className="tab-bar">
            <div className="tab-list">
                {tabs.map((tab, index) => (
                    <React.Fragment key={tab.id}>
                        {dropIndicatorIndex === index && (
                            <div className="tab-drop-indicator"/>
                        )}
                        <div
                            onDragEnter={(e) => handleDragEnter(e, index)}
                            onDragLeave={handleDragLeave}
                        >
                            <TabItem
                                tab={tab}
                                isActive={tab.id === activeTabId}
                                onTabClick={onTabClick}
                                onTabClose={onTabClose}
                                onContextMenu={onContextMenu}
                                onDragStart={handleDragStart}
                                onDragOver={handleDragOver}
                                onDrop={handleDrop}
                                index={index}
                            />
                        </div>
                    </React.Fragment>
                ))}
            </div>

            <div className="tab-actions">
                <button
                    className="tab-action"
                    onClick={onNewTab}
                    title="New tab"
                >
                    <Plus size={16}/>
                </button>
                <button
                    className="tab-action"
                    title="Tab options"
                >
                    <MoreHorizontal size={16}/>
                </button>
            </div>
        </div>
    );
};