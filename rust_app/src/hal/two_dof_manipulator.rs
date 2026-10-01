use super::{ControlMode, JointSpec, OdomKind, RobotProfile, TopicInfo, VelocityKind};

/// 2-DOF manipulator (UMNG research project IMP ING 4309).
///
/// Two revolute joints modeled after the physical prototype:
/// - `elbow_joint` — flexion/extension, mechanical range [0, 2.44] rad (~0–140°)
/// - `wrist_joint` — pronation/supination, full range [-3.14, 3.14] rad
///
/// Commanded through ros2_control's JointTrajectoryController
/// (`/arm_controller/joint_trajectory`); position feedback via the
/// joint_state_broadcaster on `/joint_states`. The same profile works
/// against the Gazebo simulation (gz_ros2_control) and, topics being
/// identical, against the physical prototype once assembled.
pub fn profile() -> RobotProfile {
    RobotProfile {
        id: "two_dof_manipulator".into(),
        name: "2DOF Manipulator".into(),
        description: "2-DOF arm (elbow flex/ext + wrist pron/sup) via ros2_control".into(),

        control_mode: ControlMode::JointPosition,

        // Velocity control does not apply to this robot: velocity-mode
        // blocks are rejected by pre-flight validation for this profile.
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
                // Literal URDF limits, NOT an approximation of π: the
                // ros2_control command interface clamps at exactly ±3.14,
                // so using PI (3.14159…) would allow out-of-range targets.
                #[allow(clippy::approx_constant)]
                min_position: -3.14,
                #[allow(clippy::approx_constant)]
                max_position: 3.14,
                max_velocity: 5.0,
            },
        ],

        odom_topic: None,
        odom_type: OdomKind::None,

        camera_topic: None,
        lidar_topic: None,

        // Not applicable in joint-position mode
        max_linear_speed: 0.0,
        max_angular_speed: 0.0,

        topics: vec![
            TopicInfo {
                name: "/arm_controller/joint_trajectory".into(),
                msg_type: "trajectory_msgs/msg/JointTrajectory".into(),
            },
            TopicInfo {
                name: "/joint_states".into(),
                msg_type: "sensor_msgs/msg/JointState".into(),
            },
            TopicInfo {
                name: "/arm_controller/controller_state".into(),
                msg_type: "control_msgs/msg/JointTrajectoryControllerState".into(),
            },
            TopicInfo {
                name: "/tf".into(),
                msg_type: "tf2_msgs/msg/TFMessage".into(),
            },
            TopicInfo {
                name: "/robot_description".into(),
                msg_type: "std_msgs/msg/String".into(),
            },
        ],
        services: vec![
            "/controller_manager/list_controllers".into(),
            "/controller_manager/load_controller".into(),
            "/controller_manager/switch_controller".into(),
        ],
        actions: vec!["/arm_controller/follow_joint_trajectory".into()],
    }
}
