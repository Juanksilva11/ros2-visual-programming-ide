pub mod turtlebot3_burger;
pub mod turtlebot3_waffle;
pub mod turtlesim;
pub mod two_dof_manipulator;

use serde::{Deserialize, Serialize};

// ─── Control Mode Abstraction ───────────────────────────────────────────────

/// How the robot is commanded. This is the top-level split in the HAL:
/// mobile bases consume velocity commands, manipulators consume joint
/// position targets. Blocks declare which mode they require and the
/// pre-flight validation rejects mismatches.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ControlMode {
    /// Twist/TwistStamped on a cmd_vel topic (differential-drive bases)
    Velocity,
    /// trajectory_msgs/JointTrajectory on a ros2_control controller topic
    JointPosition,
}

/// One actuated joint of a manipulator, with its mechanical limits.
/// Mirrors the URDF `<limit>` tag so validation rejects out-of-range
/// targets before any motion starts.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JointSpec {
    /// ROS joint name (must match the controller's `joints` list)
    pub name: String,
    /// Human-readable label for the UI (e.g. "Elbow (Flex/Ext)")
    pub label: String,
    /// Mechanical limits in radians
    pub min_position: f64,
    pub max_position: f64,
    /// Maximum joint velocity in rad/s (URDF `<limit velocity>`)
    pub max_velocity: f64,
}

// ─── Velocity Abstraction ───────────────────────────────────────────────────

/// Determines which ROS 2 message type to use when publishing velocity commands.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum VelocityKind {
    /// `geometry_msgs/msg/Twist` — used by Turtlesim
    Twist,
    /// `geometry_msgs/msg/TwistStamped` — used by Turtlebot3 (Humble+)
    TwistStamped,
}

// ─── Odometry Abstraction ───────────────────────────────────────────────────

/// Determines which ROS 2 message type to subscribe to for position feedback.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum OdomKind {
    /// `turtlesim/msg/Pose` — simple (x, y, theta)
    TurtlesimPose,
    /// `nav_msgs/msg/Odometry` — full 3D pose with quaternion orientation
    NavOdometry,
    /// No odometry available for this robot
    None,
}

/// Generic, robot-agnostic pose state extracted from any odometry source.
#[derive(Debug, Clone, Default)]
pub struct OdomState {
    pub x: f64,
    pub y: f64,
    pub yaw: f64, // radians
}

// ─── Topic Metadata ─────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TopicInfo {
    pub name: String,
    pub msg_type: String,
}

// ─── Robot Profile ──────────────────────────────────────────────────────────

/// A complete description of a robot's ROS 2 interface.
/// Adding a new robot = creating a new file that returns one of these.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RobotProfile {
    pub id: String,
    pub name: String,
    pub description: String,

    // How this robot is commanded (drives block availability + validation)
    pub control_mode: ControlMode,

    // Velocity control (ControlMode::Velocity)
    pub cmd_vel_topic: String,
    pub cmd_vel_type: VelocityKind,

    // Joint-position control (ControlMode::JointPosition)
    pub joint_command_topic: Option<String>,
    pub joint_state_topic: Option<String>,
    pub joints: Vec<JointSpec>,

    // Odometry / position feedback
    pub odom_topic: Option<String>,
    pub odom_type: OdomKind,

    // Camera — if the robot has an image topic for web_video_server
    pub camera_topic: Option<String>,

    // LiDAR — if the robot has a LaserScan topic for rosbridge streaming
    pub lidar_topic: Option<String>,

    // Actuator saturation limits (SI units per REP 103: m/s, rad/s).
    // Programs requesting speeds above these are rejected before execution:
    // a saturated actuator would silently break closed-loop trajectory
    // accuracy and invalidate positional-error measurements.
    pub max_linear_speed: f64,
    pub max_angular_speed: f64,

    // Full ROS 2 interface listing (for reference / introspection UI)
    pub topics: Vec<TopicInfo>,
    pub services: Vec<String>,
    pub actions: Vec<String>,
}

/// Auto-discovers all registered robot profiles.
pub fn get_all_profiles() -> Vec<RobotProfile> {
    vec![
        turtlesim::profile(),
        turtlebot3_burger::profile(),
        turtlebot3_waffle::profile(),
        two_dof_manipulator::profile(),
    ]
}

/// Find a profile by its ID.
pub fn get_profile_by_id(id: &str) -> Option<RobotProfile> {
    get_all_profiles().into_iter().find(|p| p.id == id)
}

/// Fixture profile for unit tests. Mirrors the TurtleBot3 Burger actuator
/// limits (0.22 m/s, 2.84 rad/s) so limit-violation cases are realistic.
///
/// `allow(dead_code)`: used from OTHER modules' test code (blocks/engine),
/// which rust-analyzer's per-module analysis flags as unused (false
/// positive — `cargo test` compiles warning-free).
#[cfg(test)]
#[allow(dead_code)]
pub(crate) fn test_profile() -> RobotProfile {
    RobotProfile {
        id: "test_bot".into(),
        name: "TestBot".into(),
        description: "Fixture profile for unit tests".into(),
        control_mode: ControlMode::Velocity,
        cmd_vel_topic: "/cmd_vel".into(),
        cmd_vel_type: VelocityKind::Twist,
        joint_command_topic: None,
        joint_state_topic: None,
        joints: vec![],
        odom_topic: Some("/odom".into()),
        odom_type: OdomKind::NavOdometry,
        camera_topic: None,
        lidar_topic: None,
        max_linear_speed: 0.22,
        max_angular_speed: 2.84,
        topics: vec![],
        services: vec![],
        actions: vec![],
    }
}

/// Fixture manipulator profile mirroring the 2-DOF arm joint limits
/// (elbow [0, 2.44] rad @ 2.0 rad/s, wrist [-3.14, 3.14] rad @ 5.0 rad/s).
/// ±3.14 are literal URDF limits, not approximations of π.
///
/// `allow(dead_code)`: cross-module test fixture (see `test_profile`).
#[cfg(test)]
#[allow(clippy::approx_constant, dead_code)]
pub(crate) fn test_arm_profile() -> RobotProfile {
    RobotProfile {
        id: "test_arm".into(),
        name: "TestArm".into(),
        description: "Fixture 2-DOF manipulator for unit tests".into(),
        control_mode: ControlMode::JointPosition,
        cmd_vel_topic: String::new(),
        cmd_vel_type: VelocityKind::Twist,
        joint_command_topic: Some("/arm_controller/joint_trajectory".into()),
        joint_state_topic: Some("/joint_states".into()),
        joints: vec![
            JointSpec {
                name: "elbow_joint".into(),
                label: "Elbow (Flex/Ext)".into(),
                min_position: 0.0,
                max_position: 2.44,
                max_velocity: 2.0,
            },
            JointSpec {
                name: "wrist_joint".into(),
                label: "Wrist (Pron/Sup)".into(),
                min_position: -3.14,
                max_position: 3.14,
                max_velocity: 5.0,
            },
        ],
        odom_topic: None,
        odom_type: OdomKind::None,
        camera_topic: None,
        lidar_topic: None,
        max_linear_speed: 0.0,
        max_angular_speed: 0.0,
        topics: vec![],
        services: vec![],
        actions: vec![],
    }
}
