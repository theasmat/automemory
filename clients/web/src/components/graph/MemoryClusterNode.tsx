import React from 'react';
import { Handle, Position } from '@xyflow/react';
import { Brain } from 'lucide-react';
import type { GraphEntity } from '../../types';

interface MemoryClusterNodeProps {
  data: {
    entity: GraphEntity;
  };
}

export const MemoryClusterNode: React.FC<MemoryClusterNodeProps> = ({ data }) => {
  const { entity } = data;

  return (
    <div className="bg-dark-900 border-2 border-gold-500/80 hover:border-gold-400 px-3 py-2 text-xs font-mono shadow-xl min-w-[170px] transition-all cursor-pointer group">
      <Handle type="target" position={Position.Top} className="!bg-gold-500 !w-2 !h-2 !rounded-none" />
      <div className="flex items-center gap-2">
        <Brain className="w-3.5 h-3.5 text-gold-500 shrink-0" />
        <div className="truncate font-bold text-gold-300 group-hover:text-gold-200">
          {entity.name}
        </div>
      </div>
      <div className="text-[10px] text-gray-400 truncate mt-1">
        Memory Anchor
      </div>
      <Handle type="source" position={Position.Bottom} className="!bg-gold-500 !w-2 !h-2 !rounded-none" />
    </div>
  );
};
