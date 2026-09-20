import React from 'react';
import { Handle, Position } from '@xyflow/react';
import { FileCode } from 'lucide-react';
import type { GraphEntity } from '../../types';

interface FileNodeProps {
  data: {
    entity: GraphEntity;
  };
}

export const FileNode: React.FC<FileNodeProps> = ({ data }) => {
  const { entity } = data;

  return (
    <div className="bg-dark-900 border-2 border-dark-600 hover:border-gold-500 px-3 py-2 text-xs font-mono shadow-lg min-w-[160px] transition-colors cursor-pointer group">
      <Handle type="target" position={Position.Top} className="!bg-gold-500 !w-2 !h-2 !rounded-none" />
      <div className="flex items-center gap-2">
        <FileCode className="w-3.5 h-3.5 text-gold-500 shrink-0" />
        <div className="truncate font-semibold text-gray-200 group-hover:text-gold-400">
          {entity.name}
        </div>
      </div>
      <div className="text-[10px] text-gray-500 truncate mt-1">
        {entity.path}
      </div>
      <Handle type="source" position={Position.Bottom} className="!bg-gold-500 !w-2 !h-2 !rounded-none" />
    </div>
  );
};
