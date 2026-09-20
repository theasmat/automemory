import React, { useState, useMemo } from 'react';
import {
  BrainCircuit,
  Filter,
  Layers,
  Sparkles,
  Search,
  ExternalLink,
} from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';
import { MemoryDetailModal } from './MemoryDetailModal';
import type { Memory, MemoryType } from '../../types';

export const MemoriesView: React.FC = () => {
  const { memories, searchQuery, setSearchQuery } = useAutoMemoryStore();
  const [selectedType, setSelectedType] = useState<string>('all');
  const [activeMemoryModal, setActiveMemoryModal] = useState<Memory | null>(null);

  const memoryTypes = ['all', 'decision', 'pattern', 'fix', 'gotcha', 'preference', 'architecture'];

  const filteredMemories = useMemo(() => {
    return memories.filter((m) => {
      const matchesType = selectedType === 'all' || m.type === selectedType;
      return matchesType;
    });
  }, [memories, selectedType]);

  return (
    <div className="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
      {/* Header & Filter Controls */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
        <div>
          <div className="flex items-center gap-2">
            <BrainCircuit className="w-5 h-5 text-gold-500" />
            <h2 className="text-xl font-bold font-mono text-white tracking-tight">
              Crystallized Memories ({filteredMemories.length})
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Persisted cross-session knowledge indexed via SQLite FTS5 BM25 and 384-dim semantic vectors.
          </p>
        </div>

        {/* Type Filter Tabs */}
        <div className="flex flex-wrap items-center gap-1 bg-dark-900 border border-dark-700 p-1">
          {memoryTypes.map((type) => (
            <button
              key={type}
              onClick={() => setSelectedType(type)}
              className={`px-3 py-1 text-xs font-mono capitalize transition-all duration-150 ${
                selectedType === type
                  ? 'bg-gold-500 text-black font-bold'
                  : 'text-gray-400 hover:text-white hover:bg-dark-800'
              }`}
            >
              {type}
            </button>
          ))}
        </div>
      </div>

      {/* Memory Cards Grid */}
      {filteredMemories.length === 0 ? (
        <div className="text-center py-16 bg-dark-950 border border-dark-700 font-mono text-xs text-gray-500">
          No memories match the selected filters or query.
        </div>
      ) : (
        <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
          {filteredMemories.map((memory) => {
            return (
              <div
                key={memory.id}
                onClick={() => setActiveMemoryModal(memory)}
                className="bg-dark-900 border border-dark-700 hover:border-gold-500/60 p-4 transition-all duration-150 flex flex-col justify-between cursor-pointer group shadow-sm hover:shadow-gold-500/5"
              >
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <span className="px-2 py-0.5 bg-dark-800 text-gold-400 font-mono font-semibold uppercase text-[10px] tracking-wider border border-dark-700">
                      {memory.type}
                    </span>
                    <div className="flex items-center gap-2 text-[10px] font-mono text-gray-500">
                      <span>v{memory.version}</span>
                      <span className="text-emerald-400 font-bold">
                        {(memory.confidence * 100).toFixed(0)}%
                      </span>
                    </div>
                  </div>

                  <p className="text-xs font-mono text-gray-200 line-clamp-4 leading-relaxed mt-2 group-hover:text-white">
                    {memory.content}
                  </p>
                </div>

                <div className="mt-4 pt-3 border-t border-dark-800 flex items-center justify-between text-[10px] font-mono text-gray-500">
                  <span>{new Date(memory.created_at).toLocaleDateString()}</span>
                  <span className="text-gold-500 opacity-0 group-hover:opacity-100 transition-opacity flex items-center gap-1">
                    Inspect <ExternalLink className="w-3 h-3" />
                  </span>
                </div>
              </div>
            );
          })}
        </div>
      )}

      {/* Inspection Modal */}
      {activeMemoryModal && (
        <MemoryDetailModal
          memory={activeMemoryModal}
          onClose={() => setActiveMemoryModal(null)}
        />
      )}
    </div>
  );
};
