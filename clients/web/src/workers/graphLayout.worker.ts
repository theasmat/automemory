import type { GraphEntity, GraphEdge } from '../types';

export interface LayoutRequest {
  type: 'CALCULATE_LAYOUT';
  entities: GraphEntity[];
  edges: GraphEdge[];
  width: number;
  height: number;
}

export interface LayoutResponse {
  type: 'LAYOUT_COMPLETE';
  nodes: Array<{
    id: string;
    x: number;
    y: number;
    entity: GraphEntity;
  }>;
  edges: GraphEdge[];
}

self.onmessage = (e: MessageEvent<LayoutRequest>) => {
  if (e.data.type === 'CALCULATE_LAYOUT') {
    const { entities, edges, width, height } = e.data;

    const nodeCount = entities.length;
    if (nodeCount === 0) {
      self.postMessage({ type: 'LAYOUT_COMPLETE', nodes: [], edges: [] } as LayoutResponse);
      return;
    }

    // Force simulation parameters
    const positions = new Map<string, { x: number; y: number; vx: number; vy: number }>();
    const centerX = width / 2;
    const centerY = height / 2;
    const radius = Math.min(width, height) * 0.42;

    // 1. Initial circular / grouped distribution based on entity type
    entities.forEach((entity, index) => {
      const angle = (index / nodeCount) * 2 * Math.PI;
      let groupOffset = 1.0;
      if (entity.entity_type === 'file') groupOffset = 0.8;
      else if (entity.entity_type === 'function') groupOffset = 1.1;
      else if (entity.entity_type === 'struct' || entity.entity_type === 'class') groupOffset = 0.95;
      else if (entity.entity_type === 'doc') groupOffset = 0.6;

      const r = radius * groupOffset;
      positions.set(entity.id, {
        x: centerX + r * Math.cos(angle) + (Math.random() - 0.5) * 40,
        y: centerY + r * Math.sin(angle) + (Math.random() - 0.5) * 40,
        vx: 0,
        vy: 0,
      });
    });

    // 2. Multi-iteration spring / repulsion physics off-thread
    const iterations = 85;
    const repulsion = 4500;
    const springLength = 120;
    const springK = 0.05;
    const damping = 0.88;

    for (let iter = 0; iter < iterations; iter++) {
      // Repulsion between all pairs
      for (let i = 0; i < nodeCount; i++) {
        const idA = entities[i].id;
        const posA = positions.get(idA)!;

        for (let j = i + 1; j < nodeCount; j++) {
          const idB = entities[j].id;
          const posB = positions.get(idB)!;

          const dx = posB.x - posA.x;
          const dy = posB.y - posA.y;
          const distSq = dx * dx + dy * dy + 10;
          const dist = Math.sqrt(distSq);

          const force = repulsion / distSq;
          const fx = (dx / dist) * force;
          const fy = (dy / dist) * force;

          posA.vx -= fx;
          posA.vy -= fy;
          posB.vx += fx;
          posB.vy += fy;
        }

        // Center gravity pull
        const cdx = centerX - posA.x;
        const cdy = centerY - posA.y;
        posA.vx += cdx * 0.002;
        posA.vy += cdy * 0.002;
      }

      // Edge spring forces
      for (const edge of edges) {
        const posSrc = positions.get(edge.source_id);
        const posTgt = positions.get(edge.target_id);
        if (posSrc && posTgt) {
          const dx = posTgt.x - posSrc.x;
          const dy = posTgt.y - posSrc.y;
          const dist = Math.sqrt(dx * dx + dy * dy) || 1;
          const displacement = dist - springLength;
          const springForce = displacement * springK;

          const fx = (dx / dist) * springForce;
          const fy = (dy / dist) * springForce;

          posSrc.vx += fx;
          posSrc.vy += fy;
          posTgt.vx -= fx;
          posTgt.vy -= fy;
        }
      }

      // Apply velocities with damping
      for (const pos of positions.values()) {
        pos.x += pos.vx;
        pos.y += pos.vy;
        pos.vx *= damping;
        pos.vy *= damping;
      }
    }

    const calculatedNodes = entities.map((entity) => {
      const pos = positions.get(entity.id)!;
      return {
        id: entity.id,
        x: Math.round(pos.x),
        y: Math.round(pos.y),
        entity,
      };
    });

    self.postMessage({
      type: 'LAYOUT_COMPLETE',
      nodes: calculatedNodes,
      edges,
    } as LayoutResponse);
  }
};
