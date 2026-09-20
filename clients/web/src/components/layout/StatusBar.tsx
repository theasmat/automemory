import React from 'react';
import { Network, Database, Shield, Zap, RefreshCw } from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';

export const StatusBar: React.FC = () => {
  const { stats, graphEntities, graphEdges, connectionState, latencyMs } = useAutoMemoryStore();

  return (
    <footer className="h-7 bg-dark-950 border-t border-dark-700 px-4 flex items-center justify-between text-[10px] font-mono text-gray-500 shrink-0 select-none z-10">
      {/* Left items */}
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1.5">
          <Database className="w-3 h-3 text-gold-500" />
          <span>SQLite WAL</span>
        </div>
        <div className="flex items-center gap-1.5">
          <Network className="w-3 h-3 text-cyan-400" />
          <span>AST Graph: {graphEntities.length} Entities / {graphEdges.length} Edges</span>
        </div>
      </div>

      {/* Right items */}
      <div className="flex items-center gap-4">
        <div className="flex items-center gap-1 text-gray-400">
          <Zap className="w-3 h-3 text-gold-500" />
          <span>FastEmbed RRF Hybrid</span>
        </div>
        <div className="flex items-center gap-1.5">
          <span
            className={`w-1.5 h-1.5 rounded-full ${
              connectionState === 'connected' ? 'bg-emerald-500' : 'bg-red-500'
            }`}
          />
          <span className="text-gray-400 uppercase">{connectionState}</span>
          {connectionState === 'connected' && <span className="text-emerald-400 font-bold">{latencyMs}ms</span>}
        </div>
      </div>
    </footer>
  );
};
