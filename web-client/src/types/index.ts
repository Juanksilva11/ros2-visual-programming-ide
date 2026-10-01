// Matches the EventType enum in rust_app/src/events.rs

export type EventType =
    | 'INFO'
    | 'WARNING'
    | 'ERROR'
    | 'STEP_START'
    | 'STEP_FINISH'
    | 'PROGRESS'
    | 'PROGRAM_FINISH'
    | 'PROGRAM_ABORT';

export interface SystemEvent {
    timestamp: string;
    event_type: EventType;
    message: string;
    metadata?: {
        current?: number;
        total?: number;
        percent?: number;
        step?: number;
        [key: string]: unknown;
    };
}

// Mirrors the HAL enums/structs serialized by GET /api/profiles
export type ControlMode = 'Velocity' | 'JointPosition';

export interface JointSpec {
    name: string;
    label: string;
    min_position: number;
    max_position: number;
    max_velocity: number;
}

export interface RobotProfile {
    id: string;
    name: string;
    description: string;
    control_mode: ControlMode;
    joints: JointSpec[];
}

export interface RobotStatus {
    connected: boolean;
    lastPing: number;
}