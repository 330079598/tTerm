import React, { useState } from 'react';
import { X, Terminal, Server, FolderOpen, Zap } from 'lucide-react';
import { Tab } from '../types/tab';

interface ConnectionDialogProps {
  isOpen: boolean;
  onClose: () => void;
  onConnect: (connection: Omit<Tab, 'id' | 'isActive'>) => void;
}

type ConnectionType = 'terminal' | 'ssh' | 'sftp' | 'serial';

interface ConnectionForm {
  type: ConnectionType;
  title: string;
  host: string;
  port: number;
  username: string;
  password: string;
  privateKey: string;
  serialPort: string;
  baudRate: number;
}

const defaultForm: ConnectionForm = {
  type: 'terminal',
  title: '',
  host: '',
  port: 22,
  username: '',
  password: '',
  privateKey: '',
  serialPort: '/dev/ttyUSB0',
  baudRate: 9600,
};

const connectionTypes = [
  { type: 'terminal' as const, label: 'Local Terminal', icon: Terminal },
  { type: 'ssh' as const, label: 'SSH Connection', icon: Server },
  { type: 'sftp' as const, label: 'SFTP Browser', icon: FolderOpen },
  { type: 'serial' as const, label: 'Serial Port', icon: Zap },
];

export const ConnectionDialog: React.FC<ConnectionDialogProps> = ({
  isOpen,
  onClose,
  onConnect
}) => {
  const [form, setForm] = useState<ConnectionForm>(defaultForm);

  if (!isOpen) return null;

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault();
    
    const connection: Omit<Tab, 'id' | 'isActive'> = {
      title: form.title || getDefaultTitle(form.type, form),
      type: form.type,
      isModified: false,
    };

    if (form.type !== 'terminal') {
      connection.connection = {
        host: form.host,
        port: form.port,
        username: form.username,
      };
    }

    onConnect(connection);
    setForm(defaultForm);
    onClose();
  };

  const getDefaultTitle = (type: ConnectionType, form: ConnectionForm): string => {
    switch (type) {
      case 'terminal':
        return 'Terminal';
      case 'ssh':
        return form.host ? `SSH: ${form.host}` : 'SSH Connection';
      case 'sftp':
        return form.host ? `SFTP: ${form.host}` : 'SFTP Browser';
      case 'serial':
        return form.serialPort ? `Serial: ${form.serialPort}` : 'Serial Connection';
      default:
        return 'New Connection';
    }
  };

  const updateForm = (updates: Partial<ConnectionForm>) => {
    setForm(prev => ({ ...prev, ...updates }));
  };

  return (
    <div className="dialog-overlay">
      <div className="dialog">
        <div className="dialog-header">
          <h2>New Connection</h2>
          <button className="dialog-close" onClick={onClose}>
            <X size={16} />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="dialog-content">
          {/* Connection Type Selection */}
          <div className="form-group">
            <label>Connection Type</label>
            <div className="connection-types">
              {connectionTypes.map(({ type, label, icon: Icon }) => (
                <button
                  key={type}
                  type="button"
                  className={`connection-type ${form.type === type ? 'active' : ''}`}
                  onClick={() => updateForm({ type })}
                >
                  <Icon size={20} />
                  <span>{label}</span>
                </button>
              ))}
            </div>
          </div>

          {/* Connection Name */}
          <div className="form-group">
            <label htmlFor="title">Connection Name</label>
            <input
              id="title"
              type="text"
              value={form.title}
              onChange={(e) => updateForm({ title: e.target.value })}
              placeholder={getDefaultTitle(form.type, form)}
            />
          </div>

          {/* SSH/SFTP specific fields */}
          {(form.type === 'ssh' || form.type === 'sftp') && (
            <>
              <div className="form-row">
                <div className="form-group">
                  <label htmlFor="host">Host</label>
                  <input
                    id="host"
                    type="text"
                    value={form.host}
                    onChange={(e) => updateForm({ host: e.target.value })}
                    placeholder="example.com"
                    required
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="port">Port</label>
                  <input
                    id="port"
                    type="number"
                    value={form.port}
                    onChange={(e) => updateForm({ port: parseInt(e.target.value) || 22 })}
                    min="1"
                    max="65535"
                  />
                </div>
              </div>

              <div className="form-group">
                <label htmlFor="username">Username</label>
                <input
                  id="username"
                  type="text"
                  value={form.username}
                  onChange={(e) => updateForm({ username: e.target.value })}
                  placeholder="username"
                />
              </div>

              <div className="form-group">
                <label htmlFor="password">Password</label>
                <input
                  id="password"
                  type="password"
                  value={form.password}
                  onChange={(e) => updateForm({ password: e.target.value })}
                  placeholder="Leave empty to use key authentication"
                />
              </div>
            </>
          )}

          {/* Serial specific fields */}
          {form.type === 'serial' && (
            <>
              <div className="form-row">
                <div className="form-group">
                  <label htmlFor="serialPort">Serial Port</label>
                  <input
                    id="serialPort"
                    type="text"
                    value={form.serialPort}
                    onChange={(e) => updateForm({ serialPort: e.target.value })}
                    placeholder="/dev/ttyUSB0"
                    required
                  />
                </div>
                <div className="form-group">
                  <label htmlFor="baudRate">Baud Rate</label>
                  <select
                    id="baudRate"
                    value={form.baudRate}
                    onChange={(e) => updateForm({ baudRate: parseInt(e.target.value) })}
                  >
                    <option value={9600}>9600</option>
                    <option value={19200}>19200</option>
                    <option value={38400}>38400</option>
                    <option value={57600}>57600</option>
                    <option value={115200}>115200</option>
                  </select>
                </div>
              </div>
            </>
          )}

          <div className="dialog-actions">
            <button type="button" onClick={onClose} className="btn-secondary">
              Cancel
            </button>
            <button type="submit" className="btn-primary">
              Connect
            </button>
          </div>
        </form>
      </div>
    </div>
  );
};