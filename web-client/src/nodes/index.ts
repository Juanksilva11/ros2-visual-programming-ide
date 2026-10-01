import type { NodeTypes, XYPosition } from '@xyflow/react';

import { MoveNode, type MoveNodeType } from './MoveNode';
import { WaitNode, type WaitNodeType } from './WaitNode';
import { RotateNode, type RotateNodeType } from './RotateNode';
import { OpenLoopNode, type OpenLoopNodeType } from './OpenLoopNode';
import { JointMoveNode, type JointMoveNodeType, type JointMoveNodeData } from './JointMoveNode';
import type { RobotProfile } from '../types';

/** Union of every canvas node. Extend this when adding a new block. */
export type AppNode = MoveNodeType | WaitNodeType | RotateNodeType | OpenLoopNodeType | JointMoveNodeType;

export type AppNodeType = NonNullable<AppNode['type']>;

/** React Flow renderer registry. */
export const nodeTypes: NodeTypes = {
    moveNode: MoveNode,
    waitNode: WaitNode,
    rotateNode: RotateNode,
    openLoopNode: OpenLoopNode,
    jointMoveNode: JointMoveNode,
};

type OnNodeDataChange = (key: string, value: string) => void;

const clamp = (value: number, min: number, max: number) => Math.min(Math.max(value, min), max);

type NodeFactory = (
    id: string,
    position: XYPosition,
    onChange: OnNodeDataChange,
    profile: RobotProfile | null,
) => AppNode | null;

/**
 * Registry of default data per block type. Adding a new block to the
 * palette = one entry here + one in `nodeTypes` + its component.
 *
 * Factories may consult the active HAL profile: the joint-move block
 * stamps the profile's joint metadata (names, labels, limits) into the
 * node so the UI can render one bounded field per joint.
 */
const NODE_FACTORIES: Record<AppNodeType, NodeFactory> = {
    moveNode: (id, position, onChange) =>
        ({ id, type: 'moveNode', position, data: { distance: 1.0, speed: 1.0, onChange } }),
    waitNode: (id, position, onChange) =>
        ({ id, type: 'waitNode', position, data: { duration: 1.0, onChange } }),
    rotateNode: (id, position, onChange) =>
        ({ id, type: 'rotateNode', position, data: { angle: 90, speed: 1.0, onChange } }),
    openLoopNode: (id, position, onChange) =>
        ({ id, type: 'openLoopNode', position, data: { linear_x: 0.5, angular_z: 0.0, duration: 2.0, onChange } }),
    jointMoveNode: (id, position, onChange, profile) => {
        if (!profile || profile.joints.length === 0) return null;
        const data: JointMoveNodeData = { duration: 3.0, joints: profile.joints, onChange };
        profile.joints.forEach((joint, i) => {
            data[`position_${i}`] = clamp(0, joint.min_position, joint.max_position);
        });
        return { id, type: 'jointMoveNode', position, data };
    },
};

/** Builds a new node with default data, or null for unknown palette types. */
export const createNode = (
    type: string,
    position: XYPosition,
    onChange: (id: string) => OnNodeDataChange,
    profile: RobotProfile | null = null,
): AppNode | null => {
    const factory = NODE_FACTORIES[type as AppNodeType];
    if (!factory) return null;
    const id = `${type}-${Date.now()}`;
    return factory(id, position, onChange(id), profile);
};
