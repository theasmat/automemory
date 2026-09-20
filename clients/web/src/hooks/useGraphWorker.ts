import { useState, useEffect, useRef } from 'react';
import type { Node, Edge } from '@xyflow/react';
import type { GraphEntity, GraphEdge } from '../types';
import type { LayoutRequest, LayoutResponse } from '../workers/graphLayout.worker';

export function useGraphWorker(entities: GraphEntity[], edges: GraphEdge[], width = 1200, height = 800) {
  const [nodes, setNodes] = useState<Node[]>([]);
  const [flowEdges, setFlowEdges] = useState<Edge[]>([]);
  const [isCalculating, setIsCalculating] = useState(false);
  const workerRef = useRef<Worker | null>(null);

  useEffect(() => {
    // Instantiate web worker using Vite worker constructor
    workerRef.current = new Worker(
      new URL('../workers/graphLayout.worker.ts', import.meta.url),
      { type: 'module' }
    );

    workerRef.current.onmessage = (event: MessageEvent<LayoutResponse>) => {
      if (event.data.type === 'LAYOUT_COMPLETE') {
        const { nodes: calculatedNodes, edges: rawEdges } = event.data;

        const reactFlowNodes: Node[] = calculatedNodes.map((item) => {
          let nodeType = 'fileNode';
          if (item.entity.entity_type === 'function') nodeType = 'functionNode';
          else if (item.entity.entity_type === 'struct' || item.entity.entity_type === 'class') nodeType = 'structNode';
          else if (item.entity.entity_type === 'memory_cluster') nodeType = 'memoryClusterNode';

          return {
            id: item.id,
            type: nodeType,
            position: { x: item.x, y: item.y },
            data: { entity: item.entity },
          };
        });

        const reactFlowEdges: Edge[] = rawEdges.map((edge, idx) => {
          let strokeColor = '#333333';
          let animated = false;
          if (edge.relation_type === 'defines') strokeColor = '#29abe2';
          else if (edge.relation_type === 'imports') strokeColor = '#ffc000';
          else if (edge.relation_type === 'modifies') {
            strokeColor = '#ef4444';
            animated = true;
          }

          return {
            id: `edge_${edge.source_id}_${edge.target_id}_${idx}`,
            source: edge.source_id,
            target: edge.target_id,
            animated,
            style: { stroke: strokeColor, strokeWidth: 1.5 },
            data: { relation_type: edge.relation_type, confidence: edge.confidence },
          };
        });

        setNodes(reactFlowNodes);
        setFlowEdges(reactFlowEdges);
        setIsCalculating(false);
      }
    };

    return () => {
      if (workerRef.current) {
        workerRef.current.terminate();
        workerRef.current = null;
      }
    };
  }, []);

  useEffect(() => {
    if (!workerRef.current || entities.length === 0) {
      setNodes([]);
      setFlowEdges([]);
      return;
    }

    setIsCalculating(true);
    const msg: LayoutRequest = {
      type: 'CALCULATE_LAYOUT',
      entities,
      edges,
      width,
      height,
    };
    workerRef.current.postMessage(msg);
  }, [entities, edges, width, height]);

  return { nodes, flowEdges, isCalculating, setNodes, setFlowEdges };
}
