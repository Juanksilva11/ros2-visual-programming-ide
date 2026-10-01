import { memo } from 'react';
import { Handle, Position } from '@xyflow/react';
import type { Node, NodeProps } from '@xyflow/react';
import { Bot } from 'lucide-react';
import { NodeNumberField } from './NodeNumberField';
import type { JointSpec } from '../types';

export type JointMoveNodeData = {
    /** Target position per joint, flat keys: position_0, position_1, … */
    [key: `position_${number}`]: number;
    /** Motion duration in seconds (JointTrajectory time_from_start) */
    duration: number;
    /** Joint metadata stamped from the active HAL profile at drop time */
    joints: JointSpec[];
    isExecuting?: boolean;
    hasError?: boolean;
    onChange?: (key: string, value: string) => void;
};

export type JointMoveNodeType = Node<JointMoveNodeData, 'jointMoveNode'>;

export const JointMoveNode = memo(({ data, isConnectable }: NodeProps<JointMoveNodeType>) => {
    return (
        <div className={`glass-panel min-w-[220px] overflow-hidden transition-all duration-300 hover:border-violet-400 hover:shadow-[0_0_15px_rgba(167,139,250,0.3)] group/node ${data.isExecuting ? 'neon-pulse-executing' : data.hasError ? 'node-error' : ''}`}>
            <div className="bg-surface/80 px-4 py-2 border-b border-white/5 flex items-center gap-2">
                <div className="p-1 bg-violet-900/30 rounded text-violet-400 shadow-[0_0_8px_rgba(167,139,250,0.3)]">
                    <Bot size={14} />
                </div>
                <span className="text-[11px] font-bold text-white uppercase tracking-widest font-mono">
                    Joint Move
                </span>
            </div>

            <div className="p-3 space-y-3">
                {data.joints.map((joint, i) => (
                    <NodeNumberField
                        key={joint.name}
                        label={`${joint.label} (rad)`}
                        accent="violet"
                        value={data[`position_${i}`] ?? 0}
                        min={joint.min_position}
                        max={joint.max_position}
                        step={0.1}
                        title={`Range: ${joint.min_position} to ${joint.max_position} rad`}
                        onValueChange={v => data.onChange?.(`position_${i}`, v)}
                    />
                ))}
                <NodeNumberField
                    label="Duration (s)"
                    accent="violet"
                    value={data.duration}
                    min={0.1}
                    step={0.5}
                    title="Time the controller takes to reach the target"
                    onValueChange={v => data.onChange?.('duration', v)}
                />
            </div>

            <Handle type="target" position={Position.Top} isConnectable={isConnectable}
                className="!bg-slate-500 !w-3 !h-3 !border-2 !border-background transition-transform hover:scale-110" />
            <Handle type="source" position={Position.Bottom} isConnectable={isConnectable}
                className="!bg-violet-400 !w-3 !h-3 !border-2 !border-background shadow-[0_0_8px_rgba(167,139,250,0.4)] transition-transform hover:scale-110" />
        </div>
    );
});
