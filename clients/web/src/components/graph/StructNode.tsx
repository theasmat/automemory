import React from 'react';
import { Handle, Position } from '@xyflow/react';
import { Box } from 'lucide-react';
import type { GraphEntity } from '../../types';

interface StructNodeProps {
  data: {
    entity: GraphEntity;
  };
}

export const StructNode: React.FC<StructNodeProps> = ({ data }) => {
  const { entity } = data;

  return (
    <div className="bg-dark-900 border border-purple-500/70 hover:border-purple-400 px-3 py-1.5 text-xs font-mono shadow-md min-w-[140px] transition-colors cursor-pointer group">
      <Handle type="target" position={Position.Top} className="!bg-purple-500 !w-1.5 !h-1.5 !rounded-none" />
      <div className="flex items-center gap-1.5">
        <Box className="w-3 h-3 text-purple-400 shrink-0" />
        <div className="truncate font-semibold text-purple-200 group-hover:text-purple-300">
          {entity.name}
        </div>
      </div>
      <div className="text-[9px] text-gray-500 truncate mt-0.5">
        {entity.entity_type}
      </div>
      <Handle type="source" position={Position.Bottom} className="!bg-purple-500 !w-1.5 !h-1.5 !rounded-none" />
    </div>
  );
};
