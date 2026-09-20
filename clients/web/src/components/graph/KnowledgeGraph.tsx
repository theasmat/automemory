import React, { useState, useMemo } from 'react';
import {
  ReactFlow,
  Controls,
  Background,
  MiniMap,
  useNodesState,
  useEdgesState,
  Node,
} from '@xyflow/react';
import '@xyflow/react/dist/style.css';
import { useAutoMemoryStore } from '../../store/useAutoMemoryStore';
import { useGraphWorker } from '../../hooks/useGraphWorker';
import { FileNode } from './FileNode';
import { FunctionNode } from './FunctionNode';
import { StructNode } from './StructNode';
import { MemoryClusterNode } from './MemoryClusterNode';
import {
  Network,
  Layers,
  AlertCircle,
  Activity,
  Maximize2,
  Filter,
} from 'lucide-react';
import type { GraphEntity } from '../../types';

const nodeTypes = {
  fileNode: FileNode,
  functionNode: FunctionNode,
  structNode: StructNode,
  memoryClusterNode: MemoryClusterNode,
};

export const KnowledgeGraph: React.FC = () => {
  const { graphEntities, graphEdges, traceImpact, impactReport, isTracingImpact } = useAutoMemoryStore();
  const [selectedEntity, setSelectedEntity] = useState<GraphEntity | null>(null);
  const [filterType, setFilterType] = useState<string>('all');

  const filteredEntities = useMemo(() => {
    if (filterType === 'all') return graphEntities;
    return graphEntities.filter((e) => e.entity_type === filterType);
  }, [graphEntities, filterType]);

  const { nodes: calculatedNodes, flowEdges, isCalculating } = useGraphWorker(
    filteredEntities,
    graphEdges,
    1400,
    900
  );

  const [nodes, , onNodesChange] = useNodesState(calculatedNodes);
  const [edges, , onEdgesChange] = useEdgesState(flowEdges);

  const handleNodeClick = (_: React.MouseEvent, node: Node) => {
    if (node.data && node.data.entity) {
      setSelectedEntity(node.data.entity as GraphEntity);
    }
  };

  const handleTraceImpact = () => {
    if (selectedEntity) {
      traceImpact(selectedEntity.path || selectedEntity.name);
    }
  };

  return (
    <div className="relative w-full h-full bg-black flex overflow-hidden">
      {/* React Flow Canvas */}
      <div className="flex-1 h-full relative">
        {isCalculating && (
          <div className="absolute top-4 left-4 z-20 bg-dark-900 border border-gold-500/50 px-3 py-1.5 flex items-center gap-2 shadow-lg">
            <div className="w-2 h-2 rounded-full bg-gold-500 animate-ping"></div>
            <span className="text-xs font-mono text-gold-400">Computing Web Worker AST Layout...</span>
          </div>
        )}

        {/* Filter Toolbar */}
        <div className="absolute top-4 right-4 z-20 bg-dark-900 border border-dark-700 p-1 flex items-center gap-1 shadow-lg">
          <Filter className="w-3.5 h-3.5 text-gray-500 ml-2" />
          {['all', 'file', 'function', 'struct', 'doc'].map((t) => (
            <button
              key={t}
              onClick={() => setFilterType(t)}
              className={`px-2.5 py-1 text-[11px] font-mono capitalize transition-colors ${
                filterType === t
                  ? 'bg-dark-700 text-gold-400 border-b-2 border-gold-500'
                  : 'text-gray-400 hover:text-white'
              }`}
            >
              {t}
            </button>
          ))}
        </div>

        <ReactFlow
          nodes={nodes.length > 0 ? nodes : calculatedNodes}
          edges={edges.length > 0 ? edges : flowEdges}
          onNodesChange={onNodesChange}
          onEdgesChange={onEdgesChange}
          onNodeClick={handleNodeClick}
          nodeTypes={nodeTypes}
          fitView
          className="bg-black"
        >
          <Background color="#1a1a1a" gap={20} />
          <Controls className="!bg-dark-900 !border-dark-700 !rounded-none" />
          <MiniMap
            nodeColor={(n) => {
              if (n.type === 'fileNode') return '#ffc000';
              if (n.type === 'functionNode') return '#29abe2';
              if (n.type === 'structNode') return '#a855f7';
              return '#444444';
            }}
            maskColor="rgba(0,0,0,0.8)"
            className="!bg-dark-950 !border-dark-700 !rounded-none"
          />
        </ReactFlow>
      </div>

      {/* Right Entity Inspector / Blast Radius Panel */}
      {selectedEntity && (
        <div className="w-80 border-l border-dark-700 bg-dark-950 p-5 flex flex-col justify-between overflow-y-auto shrink-0 z-20">
          <div>
            <div className="flex items-center justify-between pb-3 border-b border-dark-700">
              <div className="flex items-center gap-2">
                <Network className="w-4 h-4 text-gold-500" />
                <span className="text-xs font-mono font-bold text-white uppercase">Entity Inspector</span>
              </div>
              <button
                onClick={() => setSelectedEntity(null)}
                className="text-gray-500 hover:text-white text-xs font-mono"
              >
                ✕
              </button>
            </div>

            <div className="mt-4 space-y-3">
              <div>
                <span className="text-[10px] text-gray-500 font-mono uppercase">Name</span>
                <div className="text-sm font-bold font-mono text-white break-words mt-0.5">
                  {selectedEntity.name}
                </div>
              </div>

              <div>
                <span className="text-[10px] text-gray-500 font-mono uppercase">Type</span>
                <div className="text-xs font-mono text-cyan-400 mt-0.5 uppercase font-semibold">
                  {selectedEntity.entity_type}
                </div>
              </div>

              <div>
                <span className="text-[10px] text-gray-500 font-mono uppercase">File Path</span>
                <div className="text-xs font-mono text-gray-300 break-words mt-0.5 bg-dark-900 p-2 border border-dark-700">
                  {selectedEntity.path}
                </div>
              </div>

              {selectedEntity.line_start && (
                <div>
                  <span className="text-[10px] text-gray-500 font-mono uppercase">Line Range</span>
                  <div className="text-xs font-mono text-gray-300 mt-0.5">
                    L{selectedEntity.line_start} - L{selectedEntity.line_end || selectedEntity.line_start}
                  </div>
                </div>
              )}

              {/* Blast Radius Action */}
              <div className="pt-3 border-t border-dark-700">
                <button
                  onClick={handleTraceImpact}
                  disabled={isTracingImpact}
                  className="w-full flex items-center justify-center gap-2 py-2 bg-gradient-to-r from-red-600 to-amber-600 hover:from-red-500 hover:to-amber-500 text-white font-mono text-xs font-bold transition-all duration-150 disabled:opacity-50"
                >
                  <Activity className={`w-3.5 h-3.5 ${isTracingImpact ? 'animate-spin' : ''}`} />
                  <span>Trace Blast Radius (Depth 3)</span>
                </button>
              </div>

              {/* Blast Radius Results */}
              {impactReport && (
                <div className="mt-3 p-3 bg-red-950/40 border border-red-800/60 text-xs font-mono">
                  <div className="flex items-center gap-1.5 text-red-400 font-bold mb-2">
                    <AlertCircle className="w-3.5 h-3.5" />
                    <span>Blast Radius Impact</span>
                  </div>
                  <div className="text-[11px] text-gray-300 mb-2">
                    Modifying this entity directly impacts <strong className="text-white">{impactReport.affected_files.length}</strong> downstream files:
                  </div>
                  <div className="space-y-1 max-h-40 overflow-y-auto">
                    {impactReport.affected_files.map((file) => (
                      <div key={file} className="text-[10px] text-amber-300 bg-dark-900 px-1.5 py-0.5 border border-dark-700 truncate">
                        • {file}
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </div>

          <div className="pt-4 border-t border-dark-700 text-[10px] text-gray-500 font-mono text-center">
            AutoMemory AST Structural Engine
          </div>
        </div>
      )}
    </div>
  );
};
