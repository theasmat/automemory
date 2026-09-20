import React from 'react';
import { Handle, Position } from '@xyflow/react';
import { Code2 } from 'lucide-react';
import type { GraphEntity } from '../../types';

interface FunctionNodeProps {
  data: {
    entity: GraphEntity;
  };
}

export const FunctionNode: React.FC<FunctionNodeProps> = ({ data }) => {
  const { entity } = data;

  return (
    <div className="bg-dark-950 border border-cyan-500/60 hover:border-cyan-400 px-3 py-1.5 text-xs font-mono shadow-md min-w-[140px] transition-colors cursor-pointer group">
      <Handle type="target" position={Position.Top} className="!bg-cyan-500 !w-1.5 !h-1.5 !rounded-none" />
      <div className="flex items-center gap-1.5">
        <Code2 className="w-3 h-3 text-cyan-400 shrink-0" />
        <div className="truncate font-semibold text-cyan-200 group-hover:text-cyan-300">
          {entity.name}()
        </div>
      </div>
      <div className="text-[9px] text-gray-500 truncate mt-0.5">
        L{entity.line_start || 1}
      </div>
      <Handle type="source" position={Position.Bottom} className="!bg-cyan-500 !w-1.5 !h-1.5 !rounded-none" />
    </div>
  );
};
