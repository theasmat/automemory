import React, { useState } from 'react';
import {
  GitPullRequestDraft,
  CheckCircle2,
  XCircle,
  Clock,
  Sparkles,
  Layers,
  ShieldCheck,
} from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';

export const ProposalsView: React.FC = () => {
  const { proposals, decideProposal } = useAutoMemoryStore();
  const [processingId, setProcessingId] = useState<string | null>(null);

  const handleDecision = async (id: string, approved: boolean) => {
    setProcessingId(id);
    await decideProposal(id, approved);
    setProcessingId(null);
  };

  return (
    <div className="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-dark-700">
        <div>
          <div className="flex items-center gap-2">
            <GitPullRequestDraft className="w-5 h-5 text-cyan-400" />
            <h2 className="text-xl font-bold font-mono text-white tracking-tight">
              Memory Proposals Queue ({proposals.length})
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Candidate memories extracted from agent tool calls and sessions awaiting 1-click human verification.
          </p>
        </div>
      </div>

      {/* Proposal Cards */}
      {proposals.length === 0 ? (
        <div className="text-center py-20 bg-dark-950 border border-dark-700 font-mono text-xs text-gray-500 space-y-2">
          <CheckCircle2 className="w-8 h-8 text-emerald-500 mx-auto opacity-80" />
          <div className="text-gray-300 font-semibold">Queue is completely clear!</div>
          <div>All agent extracted proposals have been reviewed and committed.</div>
        </div>
      ) : (
        <div className="space-y-4">
          {proposals.map((proposal) => {
            const isProcessing = processingId === proposal.id;
            return (
              <div
                key={proposal.id}
                className="bg-dark-900 border border-dark-700 hover:border-cyan-500/50 p-5 transition-all duration-150 font-mono text-xs flex flex-col md:flex-row md:items-center justify-between gap-5 shadow-sm"
              >
                <div className="space-y-2 flex-1">
                  <div className="flex flex-wrap items-center gap-2">
                    <span className="px-2 py-0.5 bg-dark-800 text-cyan-400 uppercase font-semibold text-[10px] border border-dark-700">
                      {proposal.type}
                    </span>
                    <span className="text-gray-500 text-[10px]">
                      Source: <strong className="text-gray-400">{proposal.source_type}</strong>
                    </span>
                    <span className="text-emerald-400 text-[10px] font-bold">
                      {(proposal.confidence * 100).toFixed(0)}% Confidence
                    </span>
                    <span className="text-gray-600 text-[10px]">
                      {new Date(proposal.created_at).toLocaleString()}
                    </span>
                  </div>

                  <p className="text-sm text-gray-100 leading-relaxed bg-dark-950 p-3 border border-dark-800 whitespace-pre-wrap">
                    {proposal.proposed_content}
                  </p>
                </div>

                {/* Actions */}
                <div className="flex items-center gap-3 shrink-0">
                  <button
                    onClick={() => handleDecision(proposal.id, false)}
                    disabled={isProcessing}
                    className="flex items-center gap-1.5 px-4 py-2 bg-dark-800 hover:bg-red-950 text-gray-300 hover:text-red-400 border border-dark-700 hover:border-red-800 font-mono text-xs transition-colors disabled:opacity-50"
                  >
                    <XCircle className="w-4 h-4 text-red-500" />
                    <span>Reject</span>
                  </button>

                  <button
                    onClick={() => handleDecision(proposal.id, true)}
                    disabled={isProcessing}
                    className="flex items-center gap-1.5 px-5 py-2 bg-emerald-600 hover:bg-emerald-500 text-black font-mono text-xs font-bold transition-all shadow-md active:scale-95 disabled:opacity-50"
                  >
                    <CheckCircle2 className="w-4 h-4 text-black" />
                    <span>Approve & Crystallize</span>
                  </button>
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
