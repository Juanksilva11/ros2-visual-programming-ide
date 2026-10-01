use super::{OdomKind, RobotProfile, TopicInfo, VelocityKind};

pub fn profile() -> RobotProfile {
    RobotProfile {
        id: "turtlebot3_burger".into(),
        name: "TurtleBot3 Burger".into(),
        description: "ROBOTIS TurtleBot3 Burger — compact differential drive robot".into(),

        control_mode: super::ControlMode::Velocity,

        cmd_vel_topic: "/cmd_vel".into(),
        cmd_vel_type: VelocityKind::TwistStamped,

        joint_command_topic: None,
        joint_state_topic: None,
        joints: vec![],

        odom_topic: Some("/odom".into()),
        odom_type: OdomKind::NavOdometry,

        camera_topic: None,

        lidar_topic: Some("/scan".into()),

        // ROBOTIS TurtleBot3 Burger hardware specification
        max_linear_speed: 0.22,
        max_angular_speed: 2.84,

        topics: vec![
            TopicInfo {
                name: "/cmd_vel".into(),
                msg_type: "geometry_msgs/msg/TwistStamped".into(),
            },
            TopicInfo {
                name: "/imu".into(),
                msg_type: "sensor_msgs/msg/Imu".into(),
            },
            TopicInfo {
                name: "/joint_states".into(),
                msg_type: "sensor_msgs/msg/JointState".into(),
            },
            TopicInfo {
                name: "/odom".into(),
                msg_type: "nav_msgs/msg/Odometry".into(),
            },
            TopicInfo {
                name: "/scan".into(),
                msg_type: "sensor_msgs/msg/LaserScan".into(),
            },
            TopicInfo {
                name: "/tf".into(),
                msg_type: "tf2_msgs/msg/TFMessage".into(),
            },
            TopicInfo {
                name: "/tf_static".into(),
                msg_type: "tf2_msgs/msg/TFMessage".into(),
            },
        ],
        services: vec![
            "/robot_state_publisher/describe_parameters".into(),
            "/robot_state_publisher/get_parameter_types".into(),
            "/robot_state_publisher/get_parameters".into(),
            "/robot_state_publisher/get_type_description".into(),
            "/robot_state_publisher/list_parameters".into(),
            "/robot_state_publisher/set_parameters".into(),
            "/robot_state_publisher/set_parameters_atomically".into(),
            "/ros_gz_bridge/describe_parameters".into(),
            "/ros_gz_bridge/get_parameter_types".into(),
            "/ros_gz_bridge/get_parameters".into(),
            "/ros_gz_bridge/get_type_description".into(),
            "/ros_gz_bridge/list_parameters".into(),
            "/ros_gz_bridge/set_parameters".into(),
            "/ros_gz_bridge/set_parameters_atomically".into(),
        ],
        actions: vec![],
    }
}
