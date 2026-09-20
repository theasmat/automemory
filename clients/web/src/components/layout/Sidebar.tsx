import React from 'react';
import {
  Activity,
  Network,
  BrainCircuit,
  GitPullRequestDraft,
  Footprints,
  AlertTriangle,
  Sliders,
  Terminal,
  Zap,
} from 'lucide-react';
import { useAutoMemoryStore, TabType } from '../../store/useAutoMemoryStore';

interface NavItem {
  id: TabType;
  label: string;
  icon: React.ElementType;
  badge?: number;
  badgeColor?: string;
}

export const Sidebar: React.FC = () => {
  const { activeTab, setActiveTab, stats } = useAutoMemoryStore();

  const navItems: NavItem[] = [
    { id: 'overview', label: 'Overview', icon: Activity },
    { id: 'graph', label: 'AST Graph', icon: Network, badge: undefined },
    { id: 'memories', label: 'Memories', icon: BrainCircuit, badge: stats.active_memories, badgeColor: 'bg-gold-500 text-black' },
    { id: 'proposals', label: 'Proposals', icon: GitPullRequestDraft, badge: stats.pending_proposals, badgeColor: 'bg-cyan-500 text-black animate-pulse' },
    { id: 'traces', label: 'Traces', icon: Footprints, badge: stats.active_traces },
    { id: 'failures', label: 'Failures', icon: AlertTriangle, badge: stats.unresolved_failures, badgeColor: stats.unresolved_failures > 0 ? 'bg-red-500 text-white' : undefined },
    { id: 'context', label: 'Context Tester', icon: Sliders },
    { id: 'events', label: 'Live Ledger', icon: Terminal },
  ];

  return (
    <aside className="w-64 bg-dark-950 border-r border-dark-700 flex flex-col justify-between shrink-0 select-none z-20">
      <div>
        {/* Brand Header */}
        <div className="h-16 border-b border-dark-700 flex items-center px-5 gap-3">
          <div className="w-8 h-8 bg-gradient-to-br from-gold-500 to-amber-600 flex items-center justify-center font-bold text-black text-sm tracking-tighter">
            <Zap className="w-4 h-4 fill-black text-black" />
          </div>
          <div>
            <div className="font-bold text-sm tracking-wider text-white flex items-center gap-1.5 font-mono">
              AUTOMEMORY <span className="text-xs text-gold-500 font-extrabold">PRO</span>
            </div>
            <div className="text-[10px] text-gray-500 tracking-widest uppercase font-mono">
              Memory & AST Hybrid
            </div>
          </div>
        </div>

        {/* Navigation Items */}
        <nav className="p-3 space-y-1">
          {navItems.map((item) => {
            const Icon = item.icon;
            const isActive = activeTab === item.id;
            return (
              <button
                key={item.id}
                onClick={() => setActiveTab(item.id)}
                className={`w-full flex items-center justify-between px-3 py-2.5 text-xs font-medium font-mono transition-all duration-150 border-l-2 ${
                  isActive
                    ? 'border-gold-500 bg-dark-850 text-white shadow-sm'
                    : 'border-transparent text-gray-400 hover:text-gray-200 hover:bg-dark-900'
                }`}
              >
                <div className="flex items-center gap-3">
                  <Icon className={`w-4 h-4 ${isActive ? 'text-gold-500' : 'text-gray-500'}`} />
                  <span>{item.label}</span>
                </div>
                {item.badge !== undefined && item.badge > 0 && (
                  <span
                    className={`px-1.5 py-0.5 text-[10px] font-bold rounded-none font-mono ${
                      item.badgeColor || 'bg-dark-700 text-gray-300'
                    }`}
                  >
                    {item.badge}
                  </span>
                )}
              </button>
            );
          })}
        </nav>
      </div>

      {/* Engine Status & Sync Indicator */}
      <div className="p-4 border-t border-dark-700 bg-dark-900/50">
        <div className="flex items-center justify-between mb-2">
          <span className="text-[10px] uppercase font-mono text-gray-500">Core Runtime</span>
          <span className="text-[10px] font-mono text-cyan-400 font-semibold">Rust Native</span>
        </div>
        <div className="flex items-center justify-between">
          <span className="text-[10px] uppercase font-mono text-gray-500">Storage</span>
          <span className="text-[10px] font-mono text-gold-500 font-semibold">SQLite WAL + FTS5</span>
        </div>
      </div>
    </aside>
  );
};
