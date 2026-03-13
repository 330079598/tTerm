import React, { useEffect, useRef } from 'react';
import {
  Plus,
  X,
  Copy,
  Terminal,
  Server,
  FolderOpen,
  Zap,
} from 'lucide-react';
import { TabContextMenuAction } from '../types/tab';

interface ContextMenuProps {
  x: number;
  y: number;
  actions: TabContextMenuAction[];
  onAction: (action: string) => void;
  onClose: () => void;
}

const getActionIcon = (icon?: string) => {
  switch (icon) {
    case 'plus':
      return <Plus size={14} />;
    case 'x':
      return <X size={14} />;
    case 'copy':
      return <Copy size={14} />;
    case 'terminal':
      return <Terminal size={14} />;
    case 'server':
      return <Server size={14} />;
    case 'folder':
      return <FolderOpen size={14} />;
    case 'zap':
      return <Zap size={14} />;
    default:
      return null;
  }
};

export const ContextMenu: React.FC<ContextMenuProps> = ({
  x,
  y,
  actions,
  onAction,
  onClose
}) => {
  const menuRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (menuRef.current && !menuRef.current.contains(event.target as Node)) {
        onClose();
      }
    };

    const handleEscape = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        onClose();
      }
    };

    document.addEventListener('mousedown', handleClickOutside);
    document.addEventListener('keydown', handleEscape);

    return () => {
      document.removeEventListener('mousedown', handleClickOutside);
      document.removeEventListener('keydown', handleEscape);
    };
  }, [onClose]);

  // Adjust position to keep menu within viewport
  useEffect(() => {
    if (menuRef.current) {
      const rect = menuRef.current.getBoundingClientRect();
      const viewportWidth = window.innerWidth;
      const viewportHeight = window.innerHeight;

      let adjustedX = x;
      let adjustedY = y;

      if (x + rect.width > viewportWidth) {
        adjustedX = viewportWidth - rect.width - 10;
      }

      if (y + rect.height > viewportHeight) {
        adjustedY = viewportHeight - rect.height - 10;
      }

      menuRef.current.style.left = `${Math.max(10, adjustedX)}px`;
      menuRef.current.style.top = `${Math.max(10, adjustedY)}px`;
    }
  }, [x, y]);

  const handleAction = (action: string, disabled?: boolean) => {
    if (!disabled) {
      onAction(action);
      onClose();
    }
  };

  return (
    <div
      ref={menuRef}
      className="context-menu"
      style={{ left: x, top: y }}
    >
      {actions.map((action, index) => {
        if (action.separator) {
          return <div key={index} className="context-menu-separator" />;
        }

        return (
          <div
            key={index}
            className={`context-menu-item ${action.disabled ? 'disabled' : ''}`}
            onClick={() => handleAction(action.action, action.disabled)}
          >
            {getActionIcon(action.icon)}
            <span>{action.label}</span>
          </div>
        );
      })}
    </div>
  );
};