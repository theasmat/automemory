import type { ToolEvent } from '../types';

export interface IndexerMessage {
  type: 'INGEST_EVENTS' | 'SEARCH_EVENTS';
  events?: ToolEvent[];
  query?: string;
  toolFilter?: string;
}

let eventBuffer: ToolEvent[] = [];

self.onmessage = (e: MessageEvent<IndexerMessage>) => {
  const { type, events, query, toolFilter } = e.data;

  if (type === 'INGEST_EVENTS' && events) {
    // Append unique events by id, keeping newest first up to 1000 items
    const existingIds = new Set(eventBuffer.map((ev) => ev.id));
    for (const ev of events) {
      if (!existingIds.has(ev.id)) {
        eventBuffer.unshift(ev);
      }
    }
    if (eventBuffer.length > 1000) {
      eventBuffer = eventBuffer.slice(0, 1000);
    }
    self.postMessage({ type: 'INGEST_COMPLETE', total: eventBuffer.length });
  } else if (type === 'SEARCH_EVENTS') {
    let filtered = eventBuffer;

    if (toolFilter && toolFilter !== 'all') {
      filtered = filtered.filter((ev) => ev.tool_name.toLowerCase().includes(toolFilter.toLowerCase()));
    }

    if (query && query.trim().length > 0) {
      const q = query.toLowerCase();
      filtered = filtered.filter((ev) => {
        const payloadStr = JSON.stringify(ev.payload || {}).toLowerCase();
        return (
          ev.tool_name.toLowerCase().includes(q) ||
          payloadStr.includes(q) ||
          (ev.session_id && ev.session_id.toLowerCase().includes(q))
        );
      });
    }

    // Tool breakdown stats
    const toolCounts: Record<string, number> = {};
    for (const ev of eventBuffer) {
      toolCounts[ev.tool_name] = (toolCounts[ev.tool_name] || 0) + 1;
    }

    self.postMessage({
      type: 'SEARCH_RESULTS',
      results: filtered.slice(0, 100),
      totalMatched: filtered.length,
      toolCounts,
    });
  }
};
