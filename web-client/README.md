# web-client — visual programming IDE

Browser front end of the platform: a React 19 + TypeScript single-page
application where programs are built by connecting blocks on a
[React Flow](https://reactflow.dev) canvas, compiled into a JSON sequence
and sent to the Rust backend ([`../rust_app`](../rust_app)). See the
[root README](../README.md) for the full system.

## Requirements

- Node.js **20.19+ or 22.12+** (required by Vite 7 and Vitest 4)
- The backend running on port 3000 for anything beyond the UI itself

## Scripts

| Command | Purpose |
|---|---|
| `npm install` | Install dependencies (first time only) |
| `npm run dev` | Development server at `http://localhost:5173` |
| `npm test` | Vitest suite — 27 white-box tests of the graph evaluator |
| `npm run test:watch` | Tests in watch mode |
| `npm run lint` | ESLint |
| `npm run build` | Type-check (`tsc -b`) and production build into `dist/` |
| `npm run preview` | Serve the production build locally |

## Backend address

[`src/config.ts`](src/config.ts) is the single source of truth for every
backend URL. The host is taken from the page's own hostname
(`window.location.hostname`) with port 3000, so the IDE works on
`localhost` and when opened from another machine on the same network,
with no configuration.

## Source layout

```
src/
├── App.tsx               # composition shell (no logic of its own)
├── config.ts             # backend REST/WebSocket URLs
├── features/editor/
│   ├── compiler.ts       # graph evaluator: path-graph invariants, Kahn's
│   │                     # topological sort, parameter validation, JSON payload
│   └── compiler.test.ts  # 27 Vitest cases (topology, parameters, payload)
├── nodes/                # canvas blocks: MoveNode, RotateNode, OpenLoopNode,
│                         # WaitNode, JointMoveNode, NodeNumberField + registry
├── components/           # HeaderBar, Sidebar (palette), BottomPanel, Terminal,
│                         # RosInfoPanel, SensorSidebar, CameraPanel, LidarPanel
├── hooks/                # useRobotConnection (WebSocket), useProfiles (HAL),
│                         # useExecution, useCanvasGraph, usePanelLayout
└── types/                # TypeScript mirrors of the backend's Rust types
```

## How a program is compiled

1. **Edges are the program.** The block positions on the canvas are
   irrelevant to execution; only the connections are evaluated.
2. **Topology.** The graph must be a *path graph*: at least one block, at
   most one input and one output per block, exactly one start block, every
   block reachable from it, and no cycles. Execution order is resolved
   with Kahn's algorithm, which also detects a cycle disconnected from an
   otherwise valid chain.
3. **Parameters.** Every value must be finite and within its domain
   (non-zero distances and angles, positive speeds and durations, joint
   targets within the limits of the active robot profile).
4. **Feedback.** Any violation raises a typed `GraphValidationError` whose
   code (`CYCLE_DETECTED`, `MULTIPLE_OUTPUTS`, `INVALID_PARAMETER`, …)
   and offending blocks are highlighted in red on the canvas; nothing is
   sent to the backend.

The backend validates the whole program again against the live robot
profile before commanding any motion, so this layer is a usability aid,
not the safety guarantee.

The palette is filtered by the active robot's control mode: velocity
blocks for mobile bases, *Joint Move* for manipulators, *Wait* for both.
