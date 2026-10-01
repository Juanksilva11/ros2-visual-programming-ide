use super::{OdomKind, RobotProfile, TopicInfo, VelocityKind};

pub fn profile() -> RobotProfile {
    RobotProfile {
        id: "turtlesim".into(),
        name: "Turtlesim".into(),
        description: "ROS 2 educational 2D turtle simulator".into(),

        control_mode: super::ControlMode::Velocity,

        cmd_vel_topic: "/turtle1/cmd_vel".into(),
        cmd_vel_type: VelocityKind::Twist,

        joint_command_topic: None,
        joint_state_topic: None,
        joints: vec![],

        odom_topic: Some("/turtle1/pose".into()),
        odom_type: OdomKind::TurtlesimPose,

        camera_topic: None,

        lidar_topic: None,

        // Turtlesim has no physical actuator — these are sanity bounds
        // (canvas is ~11 units wide) to catch input mistakes.
        max_linear_speed: 10.0,
        max_angular_speed: 10.0,

        topics: vec![
            TopicInfo {
                name: "/turtle1/cmd_vel".into(),
                msg_type: "geometry_msgs/msg/Twist".into(),
            },
            TopicInfo {
                name: "/turtle1/color_sensor".into(),
                msg_type: "turtlesim/msg/Color".into(),
            },
            TopicInfo {
                name: "/turtle1/pose".into(),
                msg_type: "turtlesim/msg/Pose".into(),
            },
        ],
        services: vec![
            "/clear".into(),
            "/kill".into(),
            "/reset".into(),
            "/spawn".into(),
            "/turtle1/set_pen".into(),
            "/turtle1/teleport_absolute".into(),
            "/turtle1/teleport_relative".into(),
            "/turtlesim/describe_parameters".into(),
            "/turtlesim/get_parameter_types".into(),
            "/turtlesim/get_parameters".into(),
            "/turtlesim/get_type_description".into(),
            "/turtlesim/list_parameters".into(),
            "/turtlesim/set_parameters".into(),
            "/turtlesim/set_parameters_atomically".into(),
        ],
        actions: vec!["/turtle1/rotate_absolute".into()],
    }
}
