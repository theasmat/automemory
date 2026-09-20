import React, { useState, useEffect, useRef } from 'react';
import { Terminal, Search, Clock } from 'lucide-react';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';
import type { ToolEvent } from '../../types';

export const LiveEventLedger: React.FC = () => {
  const { recentEvents } = useAutoMemoryStore();
  const [filterQuery, setFilterQuery] = useState('');
  const [selectedTool, setSelectedTool] = useState('all');
  const [filteredEvents, setFilteredEvents] = useState<ToolEvent[]>([]);
  const [toolCounts, setToolCounts] = useState<Record<string, number>>({});
  const workerRef = useRef<Worker | null>(null);

  useEffect(() => {
    // Instantiate Event Indexer Web Worker
    workerRef.current = new Worker(
      new URL('../../workers/eventIndexer.worker.ts', import.meta.url),
      { type: 'module' }
    );

    workerRef.current.onmessage = (e) => {
      if (e.data.type === 'SEARCH_RESULTS') {
        setFilteredEvents(e.data.results || []);
        setToolCounts(e.data.toolCounts || {});
      }
    };

    return () => {
      if (workerRef.current) {
        workerRef.current.terminate();
        workerRef.current = null;
      }
    };
  }, []);

  // Ingest events into worker
  useEffect(() => {
    if (workerRef.current && recentEvents && recentEvents.length > 0) {
      workerRef.current.postMessage({
        type: 'INGEST_EVENTS',
        events: recentEvents,
      });
      workerRef.current.postMessage({
        type: 'SEARCH_EVENTS',
        query: filterQuery,
        toolFilter: selectedTool,
      });
    } else if (!recentEvents || recentEvents.length === 0) {
      setFilteredEvents([]);
    }
  }, [recentEvents]);

  // Search when filter changes
  useEffect(() => {
    if (workerRef.current) {
      workerRef.current.postMessage({
        type: 'SEARCH_EVENTS',
        query: filterQuery,
        toolFilter: selectedTool,
      });
    }
  }, [filterQuery, selectedTool]);

  const uniqueTools = ['all', ...Object.keys(toolCounts)];

  return (
    <div className="p-8 space-y-6 max-w-7xl mx-auto overflow-y-auto h-full font-mono text-xs">
      {/* Header */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4 pb-4 border-b border-dark-700">
        <div>
          <div className="flex items-center gap-2">
            <Terminal className="w-5 h-5 text-cyan-400" />
            <h2 className="text-xl font-bold font-mono text-white tracking-tight">
              Live Tool Event Ledger ({recentEvents ? recentEvents.length : 0})
            </h2>
          </div>
          <p className="text-xs font-mono text-gray-400 mt-1">
            Real-time streaming agent tool calls indexed in an off-thread Web Worker.
          </p>
        </div>

        {/* Live Filter Bar */}
        <div className="flex items-center gap-3">
          <div className="relative">
            <Search className="w-3.5 h-3.5 text-gray-500 absolute left-2.5 top-2.5" />
            <input
              type="text"
              placeholder="Filter tool payload..."
              value={filterQuery}
              onChange={(e) => setFilterQuery(e.target.value)}
              className="bg-dark-900 border border-dark-700 text-gray-200 text-xs pl-8 pr-3 py-1.5 focus:outline-none focus:border-cyan-400"
            />
          </div>
        </div>
      </div>

      {/* Tool Filter Chips */}
      <div className="flex flex-wrap gap-1 bg-dark-900 p-1.5 border border-dark-700">
        {uniqueTools.map((t) => (
          <button
            key={t}
            onClick={() => setSelectedTool(t)}
            className={`px-2.5 py-1 text-[11px] font-mono capitalize transition-colors ${
              selectedTool === t
                ? 'bg-cyan-500 text-black font-bold'
                : 'text-gray-400 hover:text-white hover:bg-dark-800'
            }`}
          >
            {t} {toolCounts[t] ? `(${toolCounts[t]})` : ''}
          </button>
        ))}
      </div>

      {/* Event Stream */}
      <div className="space-y-2">
        {!filteredEvents || filteredEvents.length === 0 ? (
          <div className="text-center py-16 bg-dark-950 border border-dark-700 text-gray-600">
            No events recorded matching criteria.
          </div>
        ) : (
          filteredEvents.map((ev, index) => {
            const eventId = ev.event_id || ev.id || `event_${index}`;
            const toolName = ev.tool?.name || ev.tool_name || ev.event_type || 'tool_call';
            const timestamp = ev.timestamp || ev.created_at || new Date().toISOString();
            const payload = ev.tool?.input || ev.raw_payload || ev.payload || {};
            const shortId = typeof eventId === 'string' ? eventId.slice(0, 8) : String(eventId);
            const status = ev.tool?.status || 'success';

            return (
              <div
                key={eventId}
                className="bg-dark-900 border border-dark-700 hover:border-cyan-500/40 p-3 space-y-2 transition-colors"
              >
                <div className="flex items-center justify-between text-[11px]">
                  <div className="flex items-center gap-2">
                    <span className="px-2 py-0.5 bg-dark-800 text-cyan-400 font-bold uppercase text-[10px] border border-dark-700">
                      {toolName}
                    </span>
                    {ev.agent?.adapter_id && (
                      <span className="px-1.5 py-0.5 bg-dark-850 text-gold-400 text-[10px] border border-dark-700">
                        {ev.agent.adapter_id}
                      </span>
                    )}
                    <span
                      className={`text-[10px] font-bold uppercase ${
                        status === 'failed' ? 'text-red-400' : 'text-emerald-400'
                      }`}
                    >
                      {status}
                    </span>
                    <span className="text-gray-500">ID: {shortId}...</span>
                  </div>
                  <div className="text-gray-500 flex items-center gap-1 text-[10px]">
                    <Clock className="w-3 h-3" />
                    <span>{new Date(timestamp).toLocaleTimeString()}</span>
                  </div>
                </div>

                <pre className="bg-dark-950 p-2.5 border border-dark-800 text-[11px] text-gray-300 overflow-x-auto leading-tight">
                  {JSON.stringify(payload, null, 2)}
                </pre>
              </div>
            );
          })
        )}
      </div>
    </div>
  );
};
