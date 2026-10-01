import { describe, it, expect } from 'vitest';
import type { Node, Edge } from '@xyflow/react';
import { compileGraphToSequence, GraphValidationError } from './compiler';

// ─── Test helpers ────────────────────────────────────────────────────────────

const node = (id: string, type: string, y: number, data: Record<string, unknown> = {}): Node =>
    ({ id, type, position: { x: 0, y }, data });

const edge = (source: string, target: string): Edge =>
    ({ id: `${source}->${target}`, source, target });

const move = (id: string, y = 0, data: Record<string, unknown> = { distance: 1, speed: 0.5 }) =>
    node(id, 'moveNode', y, data);
const wait = (id: string, y = 0, data: Record<string, unknown> = { duration: 2 }) =>
    node(id, 'waitNode', y, data);
const rotate = (id: string, y = 0, data: Record<string, unknown> = { angle: 90, speed: 1 }) =>
    node(id, 'rotateNode', y, data);
const openLoop = (id: string, y = 0, data: Record<string, unknown> = { linear_x: 0.5, angular_z: 0, duration: 2 }) =>
    node(id, 'openLoopNode', y, data);

// Joint metadata as stamped by the node factory from the 2-DOF profile
const ARM_JOINTS = [
    { name: 'elbow_joint', label: 'Elbow (Flex/Ext)', min_position: 0, max_position: 2.44, max_velocity: 2 },
    { name: 'wrist_joint', label: 'Wrist (Pron/Sup)', min_position: -3.14, max_position: 3.14, max_velocity: 5 },
];
const jointMove = (id: string, y = 0, data: Record<string, unknown> = {}) =>
    node(id, 'jointMoveNode', y, { position_0: 1.2, position_1: 1.57, duration: 3, joints: ARM_JOINTS, ...data });

/** Compiles expecting failure and returns the typed error for inspection. */
const compileError = (nodes: Node[], edges: Edge[] = []): GraphValidationError => {
    try {
        compileGraphToSequence(nodes, edges);
    } catch (error) {
        expect(error).toBeInstanceOf(GraphValidationError);
        return error as GraphValidationError;
    }
    throw new Error('Expected compilation to fail, but it succeeded');
};

// ─── White-box tests of the graph evaluator (thesis Ch. 6, unit tests) ──────

describe('graph evaluator — topological invariants (linear DAG)', () => {
    it('compiles a linear chain following the edges', () => {
        const { order, payload } = compileGraphToSequence(
            [move('A'), wait('B', 100), rotate('C', 200)],
            [edge('A', 'B'), edge('B', 'C')],
        );
        expect(order).toEqual(['A', 'B', 'C']);
        expect(payload.steps.map(s => s.type)).toEqual(['move_odometry', 'wait', 'rotate']);
    });

    it('resolves order from edges, not from canvas position', () => {
        // C is drawn ABOVE A — under the old Y-sort this would run C first
        const { order } = compileGraphToSequence(
            [move('A', 300), wait('B', 500), rotate('C', 0)],
            [edge('A', 'B'), edge('B', 'C')],
        );
        expect(order).toEqual(['A', 'B', 'C']);
    });

    it('accepts a single block as both start and end of the program', () => {
        const { order } = compileGraphToSequence([move('A')], []);
        expect(order).toEqual(['A']);
    });

    it('rejects an empty canvas', () => {
        expect(compileError([], []).code).toBe('EMPTY_PROGRAM');
    });

    it('rejects a pure cycle (no start block exists)', () => {
        const err = compileError([move('A'), wait('B')], [edge('A', 'B'), edge('B', 'A')]);
        expect(err.code).toBe('CYCLE_DETECTED');
        expect([...err.nodeIds].sort()).toEqual(['A', 'B']);
    });

    it('rejects a detached cycle and flags ONLY the cycle nodes', () => {
        // Valid chain A→B plus a separate cycle C→D→C: the canvas still has
        // exactly one start and one end, so this is the topology a naive
        // "one start, one end" rule would miss.
        const err = compileError(
            [move('A'), wait('B', 100), rotate('C', 200), move('D', 300)],
            [edge('A', 'B'), edge('C', 'D'), edge('D', 'C')],
        );
        expect(err.code).toBe('CYCLE_DETECTED');
        expect([...err.nodeIds].sort()).toEqual(['C', 'D']);
    });

    it('rejects disconnected chains and flags each chain start', () => {
        const err = compileError([move('A'), wait('B', 100), rotate('C', 200)], [edge('A', 'B')]);
        expect(err.code).toBe('DISCONNECTED_CHAINS');
        expect([...err.nodeIds].sort()).toEqual(['A', 'C']);
    });

    it('rejects fan-out (a block with two outgoing connections)', () => {
        const err = compileError([move('A'), wait('B', 100), rotate('C', 100)], [edge('A', 'B'), edge('A', 'C')]);
        expect(err.code).toBe('MULTIPLE_OUTPUTS');
        expect(err.nodeIds).toEqual(['A']);
    });

    it('rejects fan-in (a block with two incoming connections)', () => {
        const err = compileError([move('A'), wait('B'), rotate('C', 100)], [edge('A', 'C'), edge('B', 'C')]);
        expect(err.code).toBe('MULTIPLE_INPUTS');
        expect(err.nodeIds).toEqual(['C']);
    });

    it('rejects unknown block types instead of silently dropping them', () => {
        const err = compileError([node('A', 'mysteryNode', 0)]);
        expect(err.code).toBe('UNKNOWN_BLOCK');
        expect(err.nodeIds).toEqual(['A']);
    });
});

describe('graph evaluator — parameter validation', () => {
    it('rejects zero speed on Linear Move (would loop forever)', () => {
        const err = compileError([move('A', 0, { distance: 1, speed: 0 })]);
        expect(err.code).toBe('INVALID_PARAMETER');
        expect(err.nodeIds).toEqual(['A']);
    });

    it('rejects negative speed on Linear Move (speed is a magnitude)', () => {
        expect(compileError([move('A', 0, { distance: 1, speed: -0.5 })]).code).toBe('INVALID_PARAMETER');
    });

    it('rejects zero distance on Linear Move', () => {
        expect(compileError([move('A', 0, { distance: 0, speed: 0.5 })]).code).toBe('INVALID_PARAMETER');
    });

    it('accepts negative distance on Linear Move (reverse)', () => {
        const { payload } = compileGraphToSequence([move('A', 0, { distance: -1, speed: 0.5 })], []);
        expect(payload.steps[0]).toEqual({ type: 'move_odometry', target_distance: -1, speed: 0.5 });
    });

    it('rejects empty or non-numeric input', () => {
        expect(compileError([move('A', 0, { distance: '', speed: 0.5 })]).code).toBe('INVALID_PARAMETER');
        expect(compileError([move('A', 0, { distance: 'abc', speed: 0.5 })]).code).toBe('INVALID_PARAMETER');
    });

    it('rejects zero angle on Rotation (division by zero in progress)', () => {
        expect(compileError([rotate('A', 0, { angle: 0, speed: 1 })]).code).toBe('INVALID_PARAMETER');
    });

    it('accepts negative angle on Rotation (clockwise)', () => {
        const { payload } = compileGraphToSequence([rotate('A', 0, { angle: -90, speed: 1 })], []);
        expect(payload.steps[0]).toEqual({ type: 'rotate', angle_degrees: -90, speed: 1 });
    });

    it('rejects negative Wait duration but accepts zero', () => {
        expect(compileError([wait('A', 0, { duration: -1 })]).code).toBe('INVALID_PARAMETER');
        const { payload } = compileGraphToSequence([wait('A', 0, { duration: 0 })], []);
        expect(payload.steps[0]).toEqual({ type: 'wait', duration_ms: 0 });
    });

    it('rejects zero Open-Loop duration', () => {
        expect(compileError([openLoop('A', 0, { linear_x: 0.5, angular_z: 0, duration: 0 })]).code)
            .toBe('INVALID_PARAMETER');
    });

    it('accepts negative Open-Loop velocities (reverse / clockwise)', () => {
        const { payload } = compileGraphToSequence(
            [openLoop('A', 0, { linear_x: -0.5, angular_z: -0.3, duration: 2 })],
            [],
        );
        expect(payload.steps[0]).toEqual({ type: 'open_loop', linear_x: -0.5, angular_z: -0.3, duration_ms: 2000 });
    });

    it('compiles a joint move within mechanical limits to the backend schema', () => {
        const { payload } = compileGraphToSequence([jointMove('A')], []);
        expect(payload.steps[0]).toEqual({ type: 'joint_move', positions: [1.2, 1.57], duration_ms: 3000 });
    });

    it('rejects joint targets outside their mechanical range', () => {
        // Elbow above its 2.44 rad flexion limit
        const err = compileError([jointMove('A', 0, { position_0: 3.0 })]);
        expect(err.code).toBe('INVALID_PARAMETER');
        expect(err.message).toContain('Elbow');
        // Wrist beyond pronation limit
        expect(compileError([jointMove('A', 0, { position_1: 4.0 })]).code).toBe('INVALID_PARAMETER');
    });

    it('rejects joint moves with zero duration or missing joint metadata', () => {
        expect(compileError([jointMove('A', 0, { duration: 0 })]).code).toBe('INVALID_PARAMETER');
        expect(compileError([jointMove('A', 0, { joints: [] })]).code).toBe('INVALID_PARAMETER');
    });

    it('flags only the offending node in the middle of a valid chain', () => {
        const err = compileError(
            [move('A'), rotate('B', 100, { angle: 90, speed: 0 }), wait('C', 200)],
            [edge('A', 'B'), edge('B', 'C')],
        );
        expect(err.code).toBe('INVALID_PARAMETER');
        expect(err.nodeIds).toEqual(['B']);
    });
});

describe('graph evaluator — payload mapping', () => {
    it('converts seconds to milliseconds for time-based blocks', () => {
        const { payload } = compileGraphToSequence(
            [wait('A', 0, { duration: 2 }), openLoop('B', 100, { linear_x: 0.1, angular_z: 0, duration: 1.5 })],
            [edge('A', 'B')],
        );
        expect(payload.steps[0]).toEqual({ type: 'wait', duration_ms: 2000 });
        expect(payload.steps[1]).toMatchObject({ duration_ms: 1500 });
    });

    it('emits the exact backend schema for every block type', () => {
        const { payload } = compileGraphToSequence(
            [
                move('A', 0, { distance: 2, speed: 0.2 }),
                rotate('B', 100, { angle: 45, speed: 1.2 }),
                openLoop('C', 200, { linear_x: 0.1, angular_z: 0.5, duration: 3 }),
                wait('D', 300, { duration: 1 }),
            ],
            [edge('A', 'B'), edge('B', 'C'), edge('C', 'D')],
        );
        expect(payload).toEqual({
            name: 'Visual Execution',
            steps: [
                { type: 'move_odometry', target_distance: 2, speed: 0.2 },
                { type: 'rotate', angle_degrees: 45, speed: 1.2 },
                { type: 'open_loop', linear_x: 0.1, angular_z: 0.5, duration_ms: 3000 },
                { type: 'wait', duration_ms: 1000 },
            ],
        });
    });

    it('returns the node IDs in execution order for STEP-event mapping', () => {
        const { order } = compileGraphToSequence(
            [rotate('R', 500), move('M', 0)],
            [edge('R', 'M')],
        );
        expect(order).toEqual(['R', 'M']);
    });
});
