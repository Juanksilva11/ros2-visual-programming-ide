use super::{RobotProfile, TopicInfo};

pub fn profile() -> RobotProfile {
    // Start from burger and extend
    let mut p = super::turtlebot3_burger::profile();

    p.id = "turtlebot3_waffle".into();
    p.name = "TurtleBot3 Waffle".into();
    p.description = "ROBOTIS TurtleBot3 Waffle — differential drive robot with camera".into();

    // Waffle has a camera
    p.camera_topic = Some("/camera/image_raw".into());

    // ROBOTIS TurtleBot3 Waffle/Waffle Pi hardware specification
    // (overrides the Burger values inherited above)
    p.max_linear_speed = 0.26;
    p.max_angular_speed = 1.82;

    // LiDAR inherited from Burger (/scan), make explicit for clarity
    // p.lidar_topic is already Some("/scan") from burger::profile()

    // Waffle has additional camera topics
    p.topics.push(TopicInfo {
        name: "/camera/camera_info".into(),
        msg_type: "sensor_msgs/msg/CameraInfo".into(),
    });
    p.topics.push(TopicInfo {
        name: "/camera/image_raw".into(),
        msg_type: "sensor_msgs/msg/Image".into(),
    });

    // Waffle has additional ros_gz_image services
    p.services.extend([
        "/ros_gz_image/describe_parameters".into(),
        "/ros_gz_image/get_parameter_types".into(),
        "/ros_gz_image/get_parameters".into(),
        "/ros_gz_image/get_type_description".into(),
        "/ros_gz_image/list_parameters".into(),
        "/ros_gz_image/set_parameters".into(),
        "/ros_gz_image/set_parameters_atomically".into(),
    ]);

    p
}
