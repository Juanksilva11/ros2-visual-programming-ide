import type { Node, Edge } from '@xyflow/react';

// ─── Validation Errors ──────────────────────────────────────────────────────

export type GraphErrorCode =
    | 'EMPTY_PROGRAM'
    | 'MULTIPLE_OUTPUTS'
    | 'MULTIPLE_INPUTS'
    | 'DISCONNECTED_CHAINS'
    | 'CYCLE_DETECTED'
    | 'INVALID_PARAMETER'
    | 'UNKNOWN_BLOCK';

export class GraphValidationError extends Error {
    readonly code: GraphErrorCode;
    readonly nodeIds: string[];

    constructor(code: GraphErrorCode, message: string, nodeIds: string[] = []) {
        super(message);
        this.name = 'GraphValidationError';
        this.code = code;
        this.nodeIds = nodeIds;
    }
}

// ─── Program Types ──────────────────────────────────────────────────────────

type Step =
    | { type: 'move_odometry'; target_distance: number; speed: number }
    | { type: 'wait'; duration_ms: number }
    | { type: 'rotate'; angle_degrees: number; speed: number }
    | { type: 'open_loop'; linear_x: number; angular_z: number; duration_ms: number }
    | { type: 'joint_move'; positions: number[]; duration_ms: number };

export interface CompiledProgram {
    payload: { name: string; steps: Step[] };
    /** Node IDs in execution order — used to map STEP events back to canvas nodes. */
    order: string[];
}

// ─── Parameter Validation ───────────────────────────────────────────────────
//
// Generic semantic checks (finite, positive, non-zero) run here for fast
// feedback with the offending node highlighted. The backend re-validates
// every step — including robot-specific actuator limits from the HAL
// profile — as the authoritative layer, since the API can be called
// without going through this compiler.

const invalidParam = (node: Node, message: string): GraphValidationError =>
    new GraphValidationError('INVALID_PARAMETER', message, [node.id]);

const requireFinite = (node: Node, field: string): number => {
    const value = Number(node.data[field]);
    if (!Number.isFinite(value)) {
        throw invalidParam(node, `"${field}" must be a number (got "${node.data[field] ?? ''}").`);
    }
    return value;
};

const requirePositive = (node: Node, field: string): number => {
    const value = requireFinite(node, field);
    if (value <= 0) {
        throw invalidParam(node, `"${field}" must be greater than zero (got ${value}).`);
    }
    return value;
};

const requireNonNegative = (node: Node, field: string): number => {
    const value = requireFinite(node, field);
    if (value < 0) {
        throw invalidParam(node, `"${field}" cannot be negative (got ${value}).`);
    }
    return value;
};

const requireNonZero = (node: Node, field: string): number => {
    const value = requireFinite(node, field);
    if (value === 0) {
        throw invalidParam(node, `"${field}" cannot be zero.`);
    }
    return value;
};

const nodeToStep = (node: Node): Step => {
    switch (node.type) {
        // CASE 1: LINEAR MOVEMENT (Odometry-based)
        // The sign of distance sets the direction (negative = reverse),
        // mirroring the rotate block where the angle's sign does. Speed is
        // always a positive magnitude.
        case 'moveNode':
            return {
                type: 'move_odometry',
                target_distance: requireNonZero(node, 'distance'),
                speed: requirePositive(node, 'speed'),
            };

        // CASE 2: WAIT / TIMER
        case 'waitNode':
            return {
                type: 'wait',
                duration_ms: Math.round(requireNonNegative(node, 'duration') * 1000),
            };

        // CASE 3: ROTATION (Odometry-based)
        case 'rotateNode':
            return {
                type: 'rotate',
                angle_degrees: requireNonZero(node, 'angle'),
                speed: requirePositive(node, 'speed'),
            };

        // CASE 4: OPEN-LOOP MOTION (time-based velocity)
        // linear_x / angular_z may be zero or negative (reverse / clockwise)
        case 'openLoopNode':
            return {
                type: 'open_loop',
                linear_x: requireFinite(node, 'linear_x'),
                angular_z: requireFinite(node, 'angular_z'),
                duration_ms: Math.round(requirePositive(node, 'duration') * 1000),
            };

        // CASE 5: JOINT MOVE (manipulators — closed-loop via /joint_states)
        // One target per joint, checked against the mechanical limits the
        // HAL profile stamped into the node at creation. The backend
        // re-validates against the live profile as the authoritative layer.
        case 'jointMoveNode': {
            const joints = node.data.joints;
            if (!Array.isArray(joints) || joints.length === 0) {
                throw invalidParam(node, 'Joint block has no joint metadata — recreate it with a manipulator profile active.');
            }
            const positions = joints.map((joint, i) => {
                const target = requireFinite(node, `position_${i}`);
                const j = joint as { label: string; min_position: number; max_position: number };
                if (target < j.min_position || target > j.max_position) {
                    throw invalidParam(
                        node,
                        `"${j.label}" target ${target} rad is outside its mechanical range [${j.min_position}, ${j.max_position}] rad.`,
                    );
                }
                return target;
            });
            return {
                type: 'joint_move',
                positions,
                duration_ms: Math.round(requirePositive(node, 'duration') * 1000),
            };
        }

        default:
            throw new GraphValidationError(
                'UNKNOWN_BLOCK',
                `Unsupported block type: ${node.type ?? 'unknown'}.`,
                [node.id],
            );
    }
};

// ─── Graph Evaluator ────────────────────────────────────────────────────────
//
// The visual program is a DAG restricted to a path graph (strict linear
// sequencer, per the thesis scope): every node has in-degree ≤ 1 and
// out-degree ≤ 1, there is exactly one start node, and every node is
// reachable from it. Execution order is resolved with Kahn's topological
// sort, which also detects cycles (any node never reaching in-degree 0).

export const compileGraphToSequence = (nodes: Node[], edges: Edge[]): CompiledProgram => {
    if (nodes.length === 0) {
        throw new GraphValidationError('EMPTY_PROGRAM', 'Program is empty. Drag blocks onto the canvas.');
    }

    const nodeIds = new Set(nodes.map(n => n.id));
    const validEdges = edges.filter(e => nodeIds.has(e.source) && nodeIds.has(e.target));

    const nextOf = new Map<string, string[]>();
    const inDegree = new Map<string, number>();
    for (const node of nodes) {
        nextOf.set(node.id, []);
        inDegree.set(node.id, 0);
    }
    for (const edge of validEdges) {
        nextOf.get(edge.source)!.push(edge.target);
        inDegree.set(edge.target, inDegree.get(edge.target)! + 1);
    }

    // Linearity invariants: one connection per handle
    const multiOut = nodes.filter(n => nextOf.get(n.id)!.length > 1).map(n => n.id);
    if (multiOut.length > 0) {
        throw new GraphValidationError(
            'MULTIPLE_OUTPUTS',
            'A block has more than one outgoing connection. The sequencer is strictly linear.',
            multiOut,
        );
    }
    const multiIn = nodes.filter(n => inDegree.get(n.id)! > 1).map(n => n.id);
    if (multiIn.length > 0) {
        throw new GraphValidationError(
            'MULTIPLE_INPUTS',
            'A block has more than one incoming connection. The sequencer is strictly linear.',
            multiIn,
        );
    }

    // Kahn's algorithm: seed with in-degree 0 nodes (program start candidates)
    const queue = nodes.filter(n => inDegree.get(n.id) === 0).map(n => n.id);
    if (queue.length === 0) {
        throw new GraphValidationError(
            'CYCLE_DETECTED',
            'The program has no start block: the connections form a cycle.',
            nodes.map(n => n.id),
        );
    }
    if (queue.length > 1) {
        throw new GraphValidationError(
            'DISCONNECTED_CHAINS',
            `Found ${queue.length} disconnected chains. Connect all blocks into a single sequence.`,
            queue,
        );
    }

    const order: string[] = [];
    while (queue.length > 0) {
        const id = queue.shift()!;
        order.push(id);
        for (const target of nextOf.get(id)!) {
            const remaining = inDegree.get(target)! - 1;
            inDegree.set(target, remaining);
            if (remaining === 0) queue.push(target);
        }
    }

    if (order.length < nodes.length) {
        const visited = new Set(order);
        const leftover = nodes.filter(n => !visited.has(n.id)).map(n => n.id);
        throw new GraphValidationError(
            'CYCLE_DETECTED',
            'Some blocks form a cycle and can never execute.',
            leftover,
        );
    }

    const byId = new Map(nodes.map(n => [n.id, n]));
    const steps = order.map(id => nodeToStep(byId.get(id)!));

    return { payload: { name: 'Visual Execution', steps }, order };
};
