import React, { useState } from 'react';
import {
  FolderGit2,
  RefreshCw,
  Search,
  Radio,
  Wifi,
  WifiOff,
  ChevronDown,
} from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';
import { wsClient } from '../../services/wsClient';

export const Header: React.FC = () => {
  const {
    projects,
    selectedProjectId,
    setSelectedProjectId,
    searchQuery,
    setSearchQuery,
    connectionState,
    latencyMs,
    triggerSync,
  } = useAutoMemoryStore();

  const [isSyncing, setIsSyncing] = useState(false);

  const handleSync = async () => {
    setIsSyncing(true);
    await triggerSync();
    setTimeout(() => setIsSyncing(false), 800);
  };

  return (
    <header className="h-16 bg-dark-900 border-b border-dark-700 px-6 flex items-center justify-between shrink-0 select-none z-10">
      {/* Left: Project Selector & Search */}
      <div className="flex items-center gap-4 flex-1 max-w-2xl">
        {/* Project Dropdown */}
        <div className="relative flex items-center">
          <FolderGit2 className="w-4 h-4 text-gray-400 absolute left-3 pointer-events-none" />
          <select
            value={selectedProjectId || ''}
            onChange={(e) => setSelectedProjectId(e.target.value ? e.target.value : null)}
            className="bg-dark-850 border border-dark-700 text-gray-200 text-xs font-mono pl-9 pr-8 py-2 rounded-none focus:outline-none focus:border-gold-500 appearance-none cursor-pointer hover:border-dark-600 transition-colors"
          >
            <option value="">All Projects (Global)</option>
            {projects.map((p) => (
              <option key={p.id} value={p.id}>
                {p.name} ({p.slug})
              </option>
            ))}
          </select>
          <ChevronDown className="w-3.5 h-3.5 text-gray-500 absolute right-2.5 pointer-events-none" />
        </div>

        {/* Global Instant Search */}
        <div className="relative flex-1">
          <Search className="w-4 h-4 text-gray-500 absolute left-3 top-2.5 pointer-events-none" />
          <input
            type="text"
            placeholder="Search memories, AST symbols, decisions, gotchas... (RRF Hybrid)"
            value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className="w-full bg-dark-850 border border-dark-700 text-gray-200 text-xs font-mono pl-9 pr-4 py-2 rounded-none focus:outline-none focus:border-gold-500 placeholder-gray-600 transition-colors"
          />
          {searchQuery && (
            <button
              onClick={() => setSearchQuery('')}
              className="absolute right-2.5 top-2 text-xs text-gray-500 hover:text-gray-300 font-mono"
            >
              ✕
            </button>
          )}
        </div>
      </div>

      {/* Right: Sync Action & Real-Time Connection Indicator */}
      <div className="flex items-center gap-4">
        {/* Git Vault Manual Sync */}
        <button
          onClick={handleSync}
          disabled={isSyncing}
          className="flex items-center gap-2 px-3 py-1.5 bg-dark-850 hover:bg-dark-800 text-gray-300 hover:text-white border border-dark-700 text-xs font-mono transition-all duration-150 active:scale-95 disabled:opacity-50"
          title="Force Git Text Vault Markdown/JSONL Sync"
        >
          <RefreshCw className={`w-3.5 h-3.5 text-gold-500 ${isSyncing ? 'animate-spin' : ''}`} />
          <span>Sync Vault</span>
        </button>

        {/* Live WebSocket Connection Pill */}
        <div className="flex items-center gap-2.5 px-3 py-1.5 bg-dark-950 border border-dark-700">
          {connectionState === 'connected' ? (
            <>
              <span className="relative flex h-2 w-2">
                <span className="animate-ping absolute inline-flex h-full w-full rounded-full bg-emerald-400 opacity-75"></span>
                <span className="relative inline-flex rounded-full h-2 w-2 bg-emerald-500"></span>
              </span>
              <span className="text-[11px] font-mono text-gray-300">WS Live</span>
              <span className="text-[10px] font-mono text-emerald-400 font-semibold">{latencyMs}ms</span>
            </>
          ) : connectionState === 'connecting' ? (
            <>
              <Radio className="w-3.5 h-3.5 text-amber-500 animate-pulse" />
              <span className="text-[11px] font-mono text-amber-400">Connecting...</span>
            </>
          ) : (
            <button
              onClick={() => wsClient.reconnect()}
              className="flex items-center gap-1.5 text-red-400 hover:text-red-300 text-[11px] font-mono"
              title="Click to reconnect WebSocket"
            >
              <WifiOff className="w-3.5 h-3.5" />
              <span>Offline (Retry)</span>
            </button>
          )}
        </div>
      </div>
    </header>
  );
};
