# ROS 2 Low-Code Platform

A full-stack low-code robotics platform for programming ROS 2 robots from the browser using drag-and-drop block-based visual programming.

Built as an **undergraduate thesis** in Mechatronics Engineering at Universidad Militar Nueva Granada. Targets **ROS 2 Jazzy** on Ubuntu 24.04.

Programs are built by connecting blocks on a canvas; the graph is validated as a strict linear sequence (a *path graph*) and ordered with Kahn's topological sort, then executed by an asynchronous Rust engine that talks to ROS 2 natively through [r2r](https://github.com/sequenceplanner/r2r). A declarative Hardware Abstraction Layer (HAL) lets the same engine drive velocity-controlled mobile bases and joint-position-controlled manipulators.

## Supported Robots

| Robot | Control | Command | Feedback | Camera | LiDAR | Limits | Profile ID |
|---|---|---|---|---|---|---|---|
| **Turtlesim** | Velocity | `geometry_msgs/Twist` | `turtlesim/Pose` | — | — | 10.0 u/s · 10.0 rad/s¹ | `turtlesim` |
| **TurtleBot3 Burger** | Velocity | `geometry_msgs/TwistStamped` | `nav_msgs/Odometry` | — | `/scan` | 0.22 m/s · 2.84 rad/s | `turtlebot3_burger` |
| **TurtleBot3 Waffle** | Velocity | `geometry_msgs/TwistStamped` | `nav_msgs/Odometry` | `/camera/image_raw` | `/scan` | 0.26 m/s · 1.82 rad/s | `turtlebot3_waffle` |
| **2DOF Manipulator** | JointPosition | `trajectory_msgs/JointTrajectory` | `sensor_msgs/JointState` | — | — | elbow [0, 2.44] rad · wrist ±3.14 rad² | `two_dof_manipulator` |

¹ Turtlesim has no physical actuator — its limits are sanity bounds. TurtleBot3 limits come from the ROBOTIS hardware specification and are enforced by pre-execution validation: programs requesting higher speeds are rejected, since a saturated actuator would silently degrade closed-loop trajectory accuracy.

² 2-DOF arm (elbow flexion/extension + wrist pronation/supination, UMNG research project IMP ING 4309) driven through ros2_control's JointTrajectoryController. Per-joint mechanical limits and max velocities mirror the URDF and are enforced by validation (including runtime motion-speed feasibility). Validated against the authors' Gazebo simulation of the arm (the physical prototype was still being manufactured). Because the prototype exposes the same ros2_control topic interface, the profile is designed to be reused on it unchanged; this has not yet been verified on hardware.

New robots can be added by creating a profile file in `rust_app/src/hal/` implementing `RobotProfile` — velocity-controlled (with `max_linear_speed`/`max_angular_speed`) or joint-position-controlled (with `joints: Vec<JointSpec>` mechanical limits).

## Project Structure

```
├── web-client/          # React 19 + TypeScript + Vite + Tailwind v4 frontend (IDE)
│   └── src/
│       ├── config.ts    # API/WS endpoints — single source of truth
│       ├── features/editor/compiler.ts  # graph evaluator (path-graph invariants,
│       │                                # Kahn's sort, parameter validation)
│       ├── nodes/       # MoveNode, RotateNode, OpenLoopNode, WaitNode, JointMoveNode,
│       │                # NodeNumberField + node registry (index.ts)
│       ├── components/  # HeaderBar, Sidebar, BottomPanel, Terminal, RosInfoPanel,
│       │                # SensorSidebar, CameraPanel, LidarPanel
│       ├── hooks/       # useRobotConnection, useProfiles, useExecution,
│       │                # useCanvasGraph, usePanelLayout
│       └── types/       # TypeScript mirrors of the backend types
│
├── rust_app/            # Rust/Axum backend (execution engine)
│   └── src/
│       ├── api/         # REST API + WebSocket: execution, profiles, introspection,
│       │                # camera, lidar, ws (AppState and router in mod.rs)
│       ├── hal/         # Hardware Abstraction Layer: one declarative profile per robot
│       ├── blocks/      # Movement, Rotate, OpenLoop, JointMove, Wait, Log
│       ├── engine/      # ExecutionEngine (actor task, pre-flight validation,
│       │                # cooperative CancellationToken)
│       ├── robot.rs     # RobotContext: velocity/odometry and joint-position control
│       ├── events.rs    # structured events broadcast over the WebSocket
│       ├── metrics.rs   # experiment markers on /webapp_ros/markers (measurement)
│       └── main.rs      # server startup & graceful shutdown
│
├── simulation/          # Gazebo simulation of the 2-DOF manipulator (ROS 2 packages)
├── experiments/         # measurement campaign: runner, analysis, results (thesis Ch. 6)
└── .github/workflows/   # CI: backend and frontend test suites on every push
```

Each folder has its own README: [web-client](web-client/README.md), [simulation](simulation/README.md), [experiments](experiments/README.md).

## Available Blocks

| Block | Description |
|---|---|
| **Linear Move** | Moves until the target distance is covered — negative distance drives in reverse (closed-loop, odometry) |
| **Rotation** | Rotates by angle using a P-controller (closed-loop, odometry) |
| **Open-Loop** | Sends linear + angular velocity for a fixed duration (no odometry) |
| **Joint Move** | Moves a manipulator's joints to target positions over a chosen duration (closed-loop via `/joint_states`) |
| **Wait** | Pauses execution for a specified time |

The palette is filtered by the active robot's control mode: velocity blocks (Move/Rotate/Open-Loop) only appear for mobile bases, Joint Move only for manipulators, Wait for both. Cross-mode programs are also rejected by pre-flight validation.

## Features

- **Graph-Based Program Compilation**: React Flow edges define the program. The compiler validates the graph as a strict linear DAG (single start, no cycles, no disconnected chains, one connection per handle) and resolves execution order via Kahn's topological sort. Invalid topologies highlight the offending blocks in red with an error banner.
- **Three-Layer Parameter Validation**: (1) UI input constraints, (2) generic semantic checks in the frontend compiler (finite, positive, non-zero), and (3) authoritative pre-flight validation in the Rust engine — every step is checked against the active robot's HAL actuator limits *before* any motion starts, so a bad parameter can never strand the robot mid-trajectory. Rejections return HTTP 400 with the exact reason.
- **Multi-Robot HAL**: Switch between robot profiles via a header dropdown. The HAL handles message type differences (Twist vs TwistStamped, Pose vs Odometry), sensor capabilities (camera, LiDAR), and actuator saturation limits.
- **Emergency Stop**: the E-STOP button publishes a stop command immediately — zero velocity on mobile bases, a hold-at-current-position trajectory on the manipulator — and cancels the running program via a cooperative `CancellationToken`. It is a software abort mechanism, not a certified emergency-stop function.
- **Visual Execution Feedback**: Executing blocks pulse with a neon glow animation. A progress bar tracks step completion in real time.
- **Live Camera Feed**: Robots with a `camera_topic` stream video via `web_video_server` (MJPEG). The backend manages the server process lifecycle with process group control (`setsid` + `killpg`). Includes retry logic for server startup delay.
- **Real-Time LiDAR Radar**: Robots with a `lidar_topic` visualize `sensor_msgs/LaserScan` data via `rosbridge_suite` + `roslibjs`. Renders as a 2D radar on Canvas2D (concentric range rings, distance-colored scan points, forward-up orientation). Data-driven rendering at 5 Hz.
- **IDE-Style Layout**: Bottom toggle panel (Terminal + ROS Info tabs) and right sensor sidebar (Camera + LiDAR). All panels are resizable and collapsible.
- **Real-Time Terminal**: WebSocket-based log viewer showing execution progress, errors, and system events.
- **ROS 2 Introspection**: Built-in panel to scan and browse the live ROS 2 graph (topics, services, actions with `ros2 interface show` definitions). Side-by-side detail view.
- **Cyberpunk UI**: Industrial dark theme with glass panels, neon accents, and smooth micro-animations.

## Network Ports

| Port | Service | Started By |
|---|---|---|
| `5173` | Vite dev server (frontend) | `npm run dev` |
| `3000` | Axum REST API + WebSocket (backend) | `cargo run` |
| `8080` | `web_video_server` (MJPEG camera stream) | Backend on demand |
| `9090` | `rosbridge_server` (LiDAR WebSocket bridge) | Backend on demand |

Ports 8080 and 9090 are managed automatically by the backend and do not need manual startup.

## Prerequisites

| Dependency | Install |
|---|---|
| Node.js 20.19+ or 22.12+ | System package manager or [nvm](https://github.com/nvm-sh/nvm) |
| Rust & Cargo | [rustup.rs](https://rustup.rs/) |
| ROS 2 Jazzy | [docs.ros.org](https://docs.ros.org/en/jazzy/Installation.html) |
| `web_video_server` | `sudo apt install ros-jazzy-web-video-server` |
| `rosbridge_suite` | `sudo apt install ros-jazzy-rosbridge-suite` |
| TurtleBot3 simulation (optional) | `sudo apt install ros-jazzy-turtlebot3-gazebo` |
| Manipulator simulation (optional) | See [`simulation/`](simulation/README.md) |

All terminals must have the ROS 2 environment sourced: `source /opt/ros/jazzy/setup.bash`

## Running

Run each service in a separate terminal with ROS 2 sourced:

### 1. Frontend
```bash
cd web-client
npm install    # first time only
npm run dev
```

### 2. Backend
```bash
cd rust_app
cargo run
```

The frontend opens at `http://localhost:5173`. The backend API runs at `http://localhost:3000`.

## Testing

The project ships a 62-test SQA suite: 27 white-box tests of the frontend
graph evaluator (topology invariants, parameter rules, payload schema) and
35 backend tests (block validation, angle normalization, program
deserialization and pre-flight validation). CI runs both on every push
(`.github/workflows/ci.yml`).

```bash
# Frontend — 27 tests
cd web-client
npm test

# Backend — 35 tests (ROS 2 must be sourced to COMPILE r2r; no ROS node needed to run)
source /opt/ros/jazzy/setup.bash
cd rust_app
cargo test
```

## Simulation and measurement campaign

- [`simulation/`](simulation/README.md) — Gazebo model of the 2-DOF
  manipulator (UMNG research project IMP ING 4309), built as ROS 2
  packages.
- [`experiments/`](experiments/README.md) — the automated campaign of
  360 runs (12 platform–scenario combinations × 30 repetitions) used to
  validate the platform: runner, analysis pipeline, per-run results and
  the scripts that produce the thesis figures.

## API Reference

| Method | Endpoint | Description |
|---|---|---|
| **Execution** | | |
| `POST` | `/api/execute` | Execute a block sequence |
| `POST` | `/api/abort` | Emergency stop: abort + zero velocity |
| `GET` | `/ws` | WebSocket for real-time events |
| **Profiles** | | |
| `GET` | `/api/profiles` | List all robot profiles |
| `GET` | `/api/profiles/active` | Get active profile |
| `POST` | `/api/profiles/select` | Switch robot profile |
| **Introspection** | | |
| `GET` | `/api/introspect` | Discover live ROS 2 graph |
| `GET` | `/api/introspect/topic/{name}` | Topic detail + interface |
| `GET` | `/api/introspect/service/{name}` | Service detail + interface |
| `GET` | `/api/introspect/action/{name}` | Action detail + interface |
| **Camera** | | |
| `GET` | `/api/camera/status` | `{ has_camera, topic, server_running, stream_url }` |
| `POST` | `/api/camera/start` | Spawn `web_video_server` on port 8080 |
| `POST` | `/api/camera/stop` | Kill `web_video_server` process tree |
| **LiDAR** | | |
| `GET` | `/api/lidar/status` | `{ has_lidar, topic, server_running, rosbridge_url }` |
| `POST` | `/api/lidar/start` | Launch `rosbridge_server` on port 9090 |
| `POST` | `/api/lidar/stop` | Kill `rosbridge_server` process tree |

## License

Copyright 2026 Juan Camilo Silva Callejas, Fabián Parada Sepúlveda.

Licensed under the [Apache License, Version 2.0](LICENSE). Developed as an undergraduate thesis in Mechatronics Engineering at Universidad Militar Nueva Granada (Bogotá, Colombia):

> J. C. Silva Callejas and F. Parada Sepúlveda, *Desarrollo de un entorno de programación visual basado en grafos para la orquestación de trayectorias robóticas bajo el estándar ROS 2*, undergraduate thesis, Universidad Militar Nueva Granada, Bogotá, 2026. Advisors: L. E. Solaque and A. E. Velasco.
