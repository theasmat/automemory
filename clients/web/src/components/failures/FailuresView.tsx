import React from 'react';
import { AlertTriangle, Wrench } from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';

export const FailuresView: React.FC = () => {
  const { failures } = useAutoMemoryStore();

  return (
    <div className="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-dark-700">
        <div>
          <div className="flex items-center gap-2">
            <AlertTriangle className="w-5 h-5 text-red-500" />
            <h2 className="text-xl font-bold font-mono text-white tracking-tight">
              Failure & Gotchas Ledger ({failures ? failures.length : 0})
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Recorded agent traps, compilation breaks, and verified fixes to prevent repeating mistakes.
          </p>
        </div>
      </div>

      {/* Failure Cards */}
      {!failures || failures.length === 0 ? (
        <div className="text-center py-16 bg-dark-950 border border-dark-700 font-mono text-xs text-gray-500">
          No failure incidents logged. System is operating without active blockers.
        </div>
      ) : (
        <div className="space-y-4">
          {failures.map((fail) => {
            const taskTitle = fail.task_title || fail.task_description || 'Execution Incident';
            const rootCause = fail.root_cause_hypothesis || fail.root_cause;

            return (
              <div
                key={fail.id}
                className="bg-dark-900 border border-dark-700 hover:border-red-500/50 p-5 transition-all duration-150 font-mono text-xs space-y-3"
              >
                <div className="flex items-center justify-between">
                  <div className="flex items-center gap-2">
                    <span
                      className={`px-2 py-0.5 text-[10px] font-bold uppercase ${
                        fail.status === 'resolved'
                          ? 'bg-emerald-950 text-emerald-400 border border-emerald-800'
                          : 'bg-red-950 text-red-400 border border-red-800'
                      }`}
                    >
                      {fail.status || 'unresolved'}
                    </span>
                    {fail.failed_tool && (
                      <span className="px-2 py-0.5 bg-dark-800 text-cyan-400 text-[10px] border border-dark-700">
                        Tool: {fail.failed_tool}
                      </span>
                    )}
                    <span className="text-gray-500 text-[10px]">
                      {fail.created_at ? new Date(fail.created_at).toLocaleString() : ''}
                    </span>
                  </div>
                </div>

                <div>
                  <span className="text-[10px] text-gray-500 uppercase">Task Description</span>
                  <div className="text-sm font-bold text-gray-100 mt-0.5">
                    {taskTitle}
                  </div>
                </div>

                {fail.error_message && (
                  <div>
                    <span className="text-[10px] text-gray-500 uppercase">Error Output</span>
                    <div className="bg-dark-950 p-3 border border-dark-800 text-red-300 text-[11px] leading-relaxed whitespace-pre-wrap mt-0.5">
                      {fail.error_message}
                    </div>
                  </div>
                )}

                {rootCause && (
                  <div>
                    <span className="text-[10px] text-gray-500 uppercase">Root Cause Analysis</span>
                    <div className="bg-dark-950 p-2.5 border border-dark-800 text-amber-300 text-[11px] mt-0.5">
                      {rootCause}
                    </div>
                  </div>
                )}

                {fail.fix_applied && (
                  <div>
                    <span className="text-[10px] text-gray-500 uppercase">Verified Fix Applied</span>
                    <div className="bg-dark-950 p-2.5 border border-emerald-900/60 text-emerald-300 text-[11px] mt-0.5 flex items-start gap-2">
                      <Wrench className="w-3.5 h-3.5 text-emerald-400 shrink-0 mt-0.5" />
                      <span>{fail.fix_applied}</span>
                    </div>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
};
