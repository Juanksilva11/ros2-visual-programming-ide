"""Scenario and platform definitions for the measurement campaign.

Each scenario is a visual-program payload (the same JSON the frontend
compiler produces) plus recording/reset metadata. The independent
variable across S1-S4 is topological complexity (block count), per the
thesis proposal's operationalization of variables.

Speeds are chosen safely below each platform's HAL actuator limits so
pre-flight validation never rejects a campaign program.
"""

MARKER_TOPIC = "/webapp_ros/markers"

# ─── Platforms ───────────────────────────────────────────────────────────────

PLATFORMS = {
    "turtlesim": {
        "profile_id": "turtlesim",
        "topics": [MARKER_TOPIC, "/turtle1/cmd_vel", "/turtle1/pose"],
        # Turtlesim's canvas is bounded (~11x11): reset between runs so
        # drift never reaches a wall.
        "reset_cmd": ["ros2", "service", "call", "/reset", "std_srvs/srv/Empty"],
        "move_speed": 1.0,
        "move_distance": 2.0,
        "rotate_speed": 1.0,
    },
    "waffle": {
        "profile_id": "turtlebot3_waffle",
        "topics": [MARKER_TOPIC, "/cmd_vel", "/odom"],
        # Empty world, no obstacles: no reset needed — every metric is
        # measured relative to the pose at each run's start (segmented
        # by markers), so absolute drift between runs is irrelevant.
        "reset_cmd": None,
        "move_speed": 0.15,      # limit: 0.26 m/s
        "move_distance": 0.8,
        "rotate_speed": 0.5,     # limit: 1.82 rad/s
    },
    "arm": {
        "profile_id": "two_dof_manipulator",
        "topics": [MARKER_TOPIC, "/arm_controller/joint_trajectory", "/joint_states"],
        # Joint space is absolute: the reset is a homing motion issued
        # through the platform API before each measured run.
        "reset_cmd": None,
        "home_program": {
            "name": "homing",
            "steps": [{"type": "joint_move", "positions": [0.0, 0.0], "duration_ms": 3000}],
        },
    },
}

# ─── Program builders (mobile platforms) ─────────────────────────────────────


def _move(p):
    return {"type": "move_odometry", "target_distance": p["move_distance"], "speed": p["move_speed"]}


def _rotate(p, degrees):
    return {"type": "rotate", "angle_degrees": degrees, "speed": p["rotate_speed"]}


def _wait(seconds):
    return {"type": "wait", "duration_ms": int(seconds * 1000)}


def mobile_scenarios(p):
    """S1-S5 for a mobile platform parameter set `p`."""
    return {
        # S1 — minimal program: pure latency + single-segment error
        "S1_min": {"steps": [_move(p)]},
        # S2 — line with return: composed error
        "S2_line_return": {"steps": [_move(p), _rotate(p, 180), _move(p)]},
        # S3 — square: the classic accumulated-error trajectory
        "S3_square": {
            "steps": [
                _move(p), _rotate(p, 90),
                _move(p), _rotate(p, 90),
                _move(p), _rotate(p, 90),
                _move(p), _rotate(p, 90),
            ]
        },
        # S4 — long mixed program (12 blocks): latency vs complexity
        "S4_long": {
            "steps": [
                _move(p), _wait(0.5), _rotate(p, 90),
                _move(p), _wait(0.5), _rotate(p, -90),
                _move(p), _wait(0.5), _rotate(p, 180),
                _move(p), _wait(0.5), _rotate(p, -180),
            ]
        },
        # S5 — E-STOP determinism: long move aborted mid-flight
        "S5_estop": {
            "steps": [
                {
                    "type": "move_odometry",
                    "target_distance": p["move_distance"] * 3,
                    "speed": p["move_speed"],
                }
            ],
            "abort_after_s": 2.0,
        },
    }


ARM_SCENARIOS = {
    # S6 — joint-space cycle: articular error + convergence
    "S6_arm_cycle": {
        "steps": [
            {"type": "joint_move", "positions": [1.2, 1.57], "duration_ms": 3000},
            {"type": "wait", "duration_ms": 500},
            {"type": "joint_move", "positions": [2.0, -1.0], "duration_ms": 3000},
            {"type": "wait", "duration_ms": 500},
            {"type": "joint_move", "positions": [0.0, 0.0], "duration_ms": 3000},
        ]
    },
    # S7 — E-STOP on the arm: slow motion aborted mid-flight (hold check)
    "S7_arm_estop": {
        "steps": [{"type": "joint_move", "positions": [2.2, 3.0], "duration_ms": 8000}],
        "abort_after_s": 2.0,
    },
}


def get_scenarios(platform: str):
    p = PLATFORMS[platform]
    if platform == "arm":
        return ARM_SCENARIOS
    return mobile_scenarios(p)
