import React, { useState } from 'react';
import { Footprints, Clock, CheckCircle, Terminal, Layers, ChevronDown, ChevronUp } from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';

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
              Success Traces ({traces.length})
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Proven execution trajectories, tool invocation sequences, and reproducibility scores for agent tasks.
          </p>
        </div>
      </div>

      {/* Traces List */}
      {traces.length === 0 ? (
        <div className="text-center py-16 bg-dark-950 border border-dark-700 font-mono text-xs text-gray-500">
          No success traces recorded yet. Completed agent tasks will be logged automatically.
        </div>
      ) : (
        <div className="space-y-4">
          {traces.map((trace) => {
            const isExpanded = expandedId === trace.id;
            let steps: any[] = [];
            try {
              steps = JSON.parse(trace.steps_json || '[]');
            } catch {
              steps = [];
            }

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
                        {(trace.reproducibility_score * 100).toFixed(0)}% Reproducibility
                      </span>
                      <span className="text-gray-500 text-[10px]">
                        Duration: <strong className="text-gray-400">{(trace.duration_ms / 1000).toFixed(1)}s</strong>
                      </span>
                      <span className="text-gray-500 text-[10px]">
                        Tools: <strong className="text-gray-400">{trace.tools_used.join(', ') || 'native'}</strong>
                      </span>
                    </div>

                    <h3 className="text-sm font-bold text-gray-100 leading-snug">
                      {trace.task_description}
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
                      Execution Steps ({steps.length})
                    </div>
                    {steps.length === 0 ? (
                      <div className="text-gray-600 text-[11px]">No granular step details recorded.</div>
                    ) : (
                      <div className="space-y-2">
                        {steps.map((step: any, idx: number) => (
                          <div key={idx} className="bg-dark-950 p-3 border border-dark-800 space-y-1">
                            <div className="flex items-center justify-between text-[10px] text-gray-400">
                              <span className="font-bold text-gold-500">Step {step.step_number || idx + 1}</span>
                              <span>{step.timestamp || ''}</span>
                            </div>
                            <div className="text-gray-200 text-xs">{step.description}</div>
                            {step.tool_call && (
                              <div className="text-[10px] text-cyan-400 bg-dark-900 px-2 py-1 border border-dark-700 font-mono">
                                🔧 {step.tool_call}
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
