import React, { useState } from 'react';
import { Footprints, ChevronDown, ChevronUp, CheckCircle, Clock } from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';
import type { TraceStep } from '../../types';

export const TracesView: React.FC = () => {
  const { traces } = useAutoMemoryStore();
  const [expandedId, setExpandedId] = useState<string | null>(null);

  return (
    <div className="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full">
      {/* Header */}
      <div className="flex items-center justify-between pb-4 border-b border-dark-700">
        <div>
          <div className="flex items-center gap-2">
            <Footprints className="w-5 h-5 text-emerald-400" />
            <h2 className="text-xl font-bold font-mono text-white tracking-tight">
              Success Traces ({traces ? traces.length : 0})
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Proven execution trajectories, tool invocation sequences, and reproducibility scores for agent tasks.
          </p>
        </div>
      </div>

      {/* Traces List */}
      {!traces || traces.length === 0 ? (
        <div className="text-center py-16 bg-dark-950 border border-dark-700 font-mono text-xs text-gray-500">
          No success traces recorded yet. Completed agent tasks will be logged automatically.
        </div>
      ) : (
        <div className="space-y-4">
          {traces.map((trace) => {
            const isExpanded = expandedId === trace.id;

            // Normalize steps array
            let stepList: TraceStep[] = [];
            if (Array.isArray(trace.steps)) {
              stepList = trace.steps;
            } else if (typeof (trace as any).steps_json === 'string') {
              try {
                stepList = JSON.parse((trace as any).steps_json);
              } catch {
                stepList = [];
              }
            }

            const title = trace.title || (trace as any).task_description || 'Execution Trace';
            const score = typeof trace.reproducibility_score === 'number'
              ? (trace.reproducibility_score * 100).toFixed(0)
              : '100';

            // Collect unique tools used across steps
            const toolsUsed = Array.from(
              new Set(
                stepList
                  .map((s) => s.tool)
                  .filter((t): t is string => Boolean(t))
              )
            );

            return (
              <div
                key={trace.id}
                className="bg-dark-900 border border-dark-700 hover:border-emerald-500/50 p-5 transition-all duration-150 font-mono text-xs shadow-sm"
              >
                <div
                  onClick={() => setExpandedId(isExpanded ? null : trace.id)}
                  className="flex items-start justify-between cursor-pointer gap-4"
                >
                  <div className="space-y-1.5 flex-1">
                    <div className="flex flex-wrap items-center gap-2">
                      <span className="px-2 py-0.5 bg-emerald-950 text-emerald-400 font-semibold text-[10px] border border-emerald-800">
                        {score}% Reproducibility
                      </span>
                      {trace.category && (
                        <span className="px-2 py-0.5 bg-dark-800 text-gold-400 uppercase font-semibold text-[10px] border border-dark-700">
                          {trace.category}
                        </span>
                      )}
                      <span className="text-gray-500 text-[10px]">
                        Steps: <strong className="text-gray-300">{stepList.length}</strong>
                      </span>
                      {toolsUsed.length > 0 && (
                        <span className="text-gray-500 text-[10px]">
                          Tools: <strong className="text-gray-300">{toolsUsed.join(', ')}</strong>
                        </span>
                      )}
                      <span className="text-gray-600 text-[10px]">
                        {trace.created_at ? new Date(trace.created_at).toLocaleString() : ''}
                      </span>
                    </div>

                    <h3 className="text-sm font-bold text-gray-100 leading-snug">
                      {title}
                    </h3>
                  </div>

                  <button className="text-gray-500 hover:text-white p-1">
                    {isExpanded ? <ChevronUp className="w-4 h-4" /> : <ChevronDown className="w-4 h-4" />}
                  </button>
                </div>

                {/* Expanded Steps View */}
                {isExpanded && (
                  <div className="mt-4 pt-4 border-t border-dark-800 space-y-3">
                    <div className="text-[10px] text-gray-500 uppercase tracking-wider">
                      Execution Steps ({stepList.length})
                    </div>
                    {stepList.length === 0 ? (
                      <div className="text-gray-600 text-[11px]">No granular step details recorded.</div>
                    ) : (
                      <div className="space-y-2">
                        {stepList.map((step, idx) => (
                          <div key={idx} className="bg-dark-950 p-3 border border-dark-800 space-y-1.5">
                            <div className="flex items-center justify-between text-[10px] text-gray-400">
                              <span className="font-bold text-gold-500">Step {step.position || idx + 1}</span>
                              {step.tool && (
                                <span className="text-cyan-400 bg-dark-900 px-1.5 py-0.5 border border-dark-700 font-mono">
                                  🔧 {step.tool}
                                </span>
                              )}
                            </div>
                            <div className="text-gray-200 text-xs">{step.action}</div>
                            {step.command && (
                              <pre className="bg-black p-2 border border-dark-800 text-[10px] text-gray-300 font-mono overflow-x-auto">
                                $ {step.command}
                              </pre>
                            )}
                            {step.verification && (
                              <div className="text-[10px] text-emerald-400">
                                ✓ Verification: {step.verification}
                              </div>
                            )}
                          </div>
                        ))}
                      </div>
                    )}
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
