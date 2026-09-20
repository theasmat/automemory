import React from 'react';
import {
  BrainCircuit,
  GitPullRequestDraft,
  Footprints,
  AlertTriangle,
  FolderGit2,
  Zap,
  Clock,
  ChevronRight,
  ShieldAlert,
} from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';

export const OverviewView: React.FC = () => {
  const { stats, memories, proposals, failures, setActiveTab } = useAutoMemoryStore();

  const statCards = [
    {
      title: 'Active Memories',
      value: stats.active_memories,
      subtext: `${stats.archived_memories} archived`,
      icon: BrainCircuit,
      color: 'text-gold-500',
      border: 'border-gold-500/30 hover:border-gold-500',
      action: () => setActiveTab('memories'),
    },
    {
      title: 'Pending Proposals',
      value: stats.pending_proposals,
      subtext: 'Awaiting human review',
      icon: GitPullRequestDraft,
      color: 'text-cyan-400',
      border: 'border-cyan-500/30 hover:border-cyan-500',
      action: () => setActiveTab('proposals'),
      highlight: stats.pending_proposals > 0,
    },
    {
      title: 'Success Traces',
      value: stats.active_traces,
      subtext: 'Reproducible trajectories',
      icon: Footprints,
      color: 'text-emerald-400',
      border: 'border-emerald-500/30 hover:border-emerald-500',
      action: () => setActiveTab('traces'),
    },
    {
      title: 'Unresolved Failures',
      value: stats.unresolved_failures,
      subtext: 'Known gotchas & bugs',
      icon: AlertTriangle,
      color: 'text-red-400',
      border: 'border-red-500/30 hover:border-red-500',
      action: () => setActiveTab('failures'),
      highlight: stats.unresolved_failures > 0,
    },
  ];

  return (
    <div className="p-8 space-y-8 max-w-7xl mx-auto overflow-y-auto h-full">
      {/* Welcome Banner */}
      <div className="relative p-6 bg-gradient-to-r from-dark-900 via-dark-850 to-dark-900 border border-dark-700 overflow-hidden">
        <div className="relative z-10 flex items-center justify-between">
          <div>
            <div className="flex items-center gap-2 mb-2">
              <span className="px-2 py-0.5 bg-gold-500 text-black text-[10px] font-extrabold font-mono tracking-wider">
                ACTIVE DAEMON
              </span>
              <span className="text-xs font-mono text-gray-400">Zero-latency out-of-process engine</span>
            </div>
            <h1 className="text-2xl font-bold font-mono text-white tracking-tight">
              AutoMemory Pro Dashboard
            </h1>
            <p className="text-xs font-mono text-gray-400 mt-1 max-w-2xl">
              Deterministic cross-session AI memory, local 384-dim semantic embeddings, Reciprocal Rank Fusion, and real-time codebase AST knowledge graph.
            </p>
          </div>
          <div className="hidden md:flex flex-col items-end">
            <span className="text-xs font-mono text-gray-500">WebSocket Transport</span>
            <span className="text-sm font-mono text-gold-400 font-bold">ws://127.0.0.1:4545/ws</span>
          </div>
        </div>
      </div>

      {/* Metric Cards Grid */}
      <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-4">
        {statCards.map((card) => {
          const Icon = card.icon;
          return (
            <div
              key={card.title}
              onClick={card.action}
              className={`p-5 bg-dark-900 border ${card.border} transition-all duration-200 cursor-pointer group flex flex-col justify-between ${
                card.highlight ? 'ring-1 ring-gold-500/40' : ''
              }`}
            >
              <div className="flex items-center justify-between">
                <span className="text-xs font-mono text-gray-400 font-medium">{card.title}</span>
                <Icon className={`w-5 h-5 ${card.color} group-hover:scale-110 transition-transform`} />
              </div>
              <div className="mt-4">
                <div className="text-3xl font-extrabold font-mono text-white">{card.value}</div>
                <div className="text-[11px] font-mono text-gray-500 mt-1 flex items-center justify-between">
                  <span>{card.subtext}</span>
                  <ChevronRight className="w-3.5 h-3.5 text-gray-600 group-hover:text-gold-500 transition-colors" />
                </div>
              </div>
            </div>
          );
        })}
      </div>

      {/* 2-Column Split: Pending Proposals & Recent Memories */}
      <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
        {/* Pending Proposals Quick Review */}
        <div className="bg-dark-900 border border-dark-700 p-5 flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between pb-3 border-b border-dark-700">
              <div className="flex items-center gap-2">
                <GitPullRequestDraft className="w-4 h-4 text-cyan-400" />
                <h3 className="text-xs font-mono font-bold text-white uppercase">
                  Pending Memory Proposals ({proposals.length})
                </h3>
              </div>
              <button
                onClick={() => setActiveTab('proposals')}
                className="text-xs font-mono text-cyan-400 hover:text-cyan-300"
              >
                View All →
              </button>
            </div>

            <div className="mt-4 space-y-3">
              {proposals.length === 0 ? (
                <div className="text-center py-8 text-gray-600 font-mono text-xs">
                  No pending proposals. All agent memories auto-approved or consolidated.
                </div>
              ) : (
                proposals.slice(0, 3).map((p) => (
                  <div key={p.id} className="p-3 bg-dark-950 border border-dark-700 text-xs font-mono space-y-1.5">
                    <div className="flex items-center justify-between">
                      <span className="px-1.5 py-0.5 bg-dark-800 text-cyan-400 uppercase text-[10px] font-semibold">
                        {p.type}
                      </span>
                      <span className="text-gray-500 text-[10px]">
                        Confidence: {(p.confidence * 100).toFixed(0)}%
                      </span>
                    </div>
                    <p className="text-gray-300 line-clamp-2 text-[11px] leading-relaxed">
                      {p.proposed_content}
                    </p>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>

        {/* Recently Crystallized Memories */}
        <div className="bg-dark-900 border border-dark-700 p-5 flex flex-col justify-between">
          <div>
            <div className="flex items-center justify-between pb-3 border-b border-dark-700">
              <div className="flex items-center gap-2">
                <BrainCircuit className="w-4 h-4 text-gold-500" />
                <h3 className="text-xs font-mono font-bold text-white uppercase">
                  Recently Crystallized Memories
                </h3>
              </div>
              <button
                onClick={() => setActiveTab('memories')}
                className="text-xs font-mono text-gold-400 hover:text-gold-300"
              >
                View All →
              </button>
            </div>

            <div className="mt-4 space-y-3">
              {memories.length === 0 ? (
                <div className="text-center py-8 text-gray-600 font-mono text-xs">
                  No memories stored yet. Run coding tasks to capture lessons & decisions!
                </div>
              ) : (
                memories.slice(0, 3).map((m) => (
                  <div key={m.id} className="p-3 bg-dark-950 border border-dark-700 text-xs font-mono space-y-1.5">
                    <div className="flex items-center justify-between">
                      <span className="px-1.5 py-0.5 bg-dark-800 text-gold-400 uppercase text-[10px] font-semibold">
                        {m.type}
                      </span>
                      <span className="text-gray-500 text-[10px]">
                        v{m.version} • {(m.confidence * 100).toFixed(0)}% Conf
                      </span>
                    </div>
                    <p className="text-gray-300 line-clamp-2 text-[11px] leading-relaxed">
                      {m.content}
                    </p>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      </div>
    </div>
  );
};
