import React from 'react';
import { BrainCircuit, Clock, ShieldCheck, Tag, GitCommit, Layers } from 'lucide-react';
import type { Memory } from '../../types';

interface MemoryDetailModalProps {
  memory: Memory;
  onClose: () => void;
}

export const MemoryDetailModal: React.FC<MemoryDetailModalProps> = ({ memory, onClose }) => {
  return (
    <div className="fixed inset-0 z-50 bg-black/80 backdrop-blur-sm flex items-center justify-center p-4">
      <div className="bg-dark-950 border border-dark-600 max-w-2xl w-full p-6 space-y-5 shadow-2xl font-mono text-xs">
        {/* Header */}
        <div className="flex items-center justify-between pb-3 border-b border-dark-700">
          <div className="flex items-center gap-2">
            <BrainCircuit className="w-5 h-5 text-gold-500" />
            <span className="text-sm font-bold text-white uppercase">Memory Inspection</span>
            <span className="px-2 py-0.5 bg-dark-800 text-gold-400 font-semibold uppercase text-[10px]">
              {memory.type}
            </span>
          </div>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-white text-sm"
          >
            ✕
          </button>
        </div>

        {/* Content */}
        <div className="space-y-4">
          <div>
            <label className="text-[10px] text-gray-500 uppercase tracking-wider block mb-1">
              Memory Content
            </label>
            <div className="bg-dark-900 border border-dark-700 p-4 text-gray-100 text-xs leading-relaxed whitespace-pre-wrap selection:bg-gold-500 selection:text-black">
              {memory.content}
            </div>
          </div>

          {/* Metadata Grid */}
          <div className="grid grid-cols-2 md:grid-cols-4 gap-3 bg-dark-900/60 p-3 border border-dark-700">
            <div>
              <span className="text-[10px] text-gray-500 block uppercase">Confidence</span>
              <span className="text-xs font-bold text-emerald-400">
                {(memory.confidence * 100).toFixed(0)}%
              </span>
            </div>
            <div>
              <span className="text-[10px] text-gray-500 block uppercase">Version</span>
              <span className="text-xs font-bold text-cyan-400">v{memory.version}</span>
            </div>
            <div>
              <span className="text-[10px] text-gray-500 block uppercase">Status</span>
              <span className="text-xs font-bold text-gold-400 uppercase">{memory.status}</span>
            </div>
            <div>
              <span className="text-[10px] text-gray-500 block uppercase">Scope</span>
              <span className="text-xs font-bold text-gray-300">
                {memory.project_id ? 'Project-Specific' : 'Global'}
              </span>
            </div>
          </div>

          {/* Timestamps & IDs */}
          <div className="space-y-1.5 text-[10px] text-gray-500">
            <div><strong>Memory ID:</strong> <span className="text-gray-400">{memory.id}</span></div>
            <div><strong>Created:</strong> <span className="text-gray-400">{new Date(memory.created_at).toLocaleString()}</span></div>
            <div><strong>Last Updated:</strong> <span className="text-gray-400">{new Date(memory.updated_at).toLocaleString()}</span></div>
            {memory.superseded_by && (
              <div><strong>Superseded By:</strong> <span className="text-red-400">{memory.superseded_by}</span></div>
            )}
          </div>
        </div>

        {/* Footer */}
        <div className="pt-3 border-t border-dark-700 flex justify-end">
          <button
            onClick={onClose}
            className="px-4 py-1.5 bg-dark-800 hover:bg-dark-700 text-white font-mono text-xs border border-dark-600"
          >
            Close
          </button>
        </div>
      </div>
    </div>
  );
};
