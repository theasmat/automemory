import type { ToolEvent } from '../types';

export interface IndexerMessage {
  type: 'INGEST_EVENTS' | 'SEARCH_EVENTS';
  events?: ToolEvent[];
  query?: string;
  toolFilter?: string;
}

let eventBuffer: ToolEvent[] = [];

function getEventKey(ev: ToolEvent): string {
  return ev.event_id || ev.id || `${ev.timestamp || ''}_${ev.tool?.name || ev.tool_name || ''}_${Math.random()}`;
}

function getToolIdentifier(ev: ToolEvent): string {
  return ev.tool?.name || ev.tool_name || ev.event_type || 'system';
}

self.onmessage = (e: MessageEvent<IndexerMessage>) => {
  const { type, events, query, toolFilter } = e.data;

  if (type === 'INGEST_EVENTS' && events) {
    const existingKeys = new Set(eventBuffer.map(getEventKey));
    for (const ev of events) {
      const key = getEventKey(ev);
      if (!existingKeys.has(key)) {
        eventBuffer.unshift(ev);
        existingKeys.add(key);
      }
    }
    if (eventBuffer.length > 1000) {
      eventBuffer = eventBuffer.slice(0, 1000);
    }
    self.postMessage({ type: 'INGEST_COMPLETE', total: eventBuffer.length });
  } else if (type === 'SEARCH_EVENTS') {
    let filtered = eventBuffer;

    if (toolFilter && toolFilter !== 'all') {
      const filterLower = toolFilter.toLowerCase();
      filtered = filtered.filter((ev) => getToolIdentifier(ev).toLowerCase().includes(filterLower));
    }

    if (query && query.trim().length > 0) {
      const q = query.toLowerCase();
      filtered = filtered.filter((ev) => {
        const toolStr = getToolIdentifier(ev).toLowerCase();
        const payloadStr = JSON.stringify(ev.tool?.input || ev.raw_payload || ev.payload || {}).toLowerCase();
        const sessionStr = (ev.session_id || '').toLowerCase();
        return toolStr.includes(q) || payloadStr.includes(q) || sessionStr.includes(q);
      });
    }

    // Tool breakdown stats
    const toolCounts: Record<string, number> = {};
    for (const ev of eventBuffer) {
      const toolName = getToolIdentifier(ev);
      toolCounts[toolName] = (toolCounts[toolName] || 0) + 1;
    }

    self.postMessage({
      type: 'SEARCH_RESULTS',
      results: filtered.slice(0, 100),
      totalMatched: filtered.length,
      toolCounts,
    });
  }
};
