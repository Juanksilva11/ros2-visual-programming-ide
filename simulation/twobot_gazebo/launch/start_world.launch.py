import os
from ament_index_python.packages import get_package_share_directory, get_package_prefix
from launch import LaunchDescription
from launch.actions import IncludeLaunchDescription, AppendEnvironmentVariable
from launch.launch_description_sources import PythonLaunchDescriptionSource
from launch_ros.actions import Node
from launch.substitutions import Command
from launch_ros.parameter_descriptions import ParameterValue

def generate_launch_description():
    pkg_ros_gz_sim = get_package_share_directory('ros_gz_sim')
    
    description_pkg_name = 'twobot_description'
    description_pkg_share = get_package_share_directory(description_pkg_name)
    description_install_dir = get_package_prefix(description_pkg_name)

    gazebo_pkg_name = 'twobot_gazebo'
    gazebo_pkg_share = get_package_share_directory(gazebo_pkg_name)

    gz_resource_path = os.path.join(description_install_dir, 'share')
    env_var_resources = AppendEnvironmentVariable(
        'GZ_SIM_RESOURCE_PATH',
        gz_resource_path
    )

    world_file_path = os.path.join(gazebo_pkg_share, 'worlds', 'twobot_empty.sdf')
    urdf_file_path = os.path.join(description_pkg_share, 'urdf', 'twobot_v1.urdf')

    gazebo = IncludeLaunchDescription(
        PythonLaunchDescriptionSource(
            os.path.join(pkg_ros_gz_sim, 'launch', 'gz_sim.launch.py')
        ),
        launch_arguments={'gz_args': f'-r {world_file_path}'}.items(),
    )

    # LA MAGIA ESTÁ AQUÍ: Usamos 'xacro' en lugar de 'cat'
    robot_desc_content = Command(['xacro ', urdf_file_path])
    robot_description_param = {'robot_description': ParameterValue(robot_desc_content, value_type=str)}

    robot_state_publisher_node = Node(
        package='robot_state_publisher',
        executable='robot_state_publisher',
        name='robot_state_publisher',
        output='screen',
        parameters=[robot_description_param]
    )

    spawn_entity = Node(
        package='ros_gz_sim',
        executable='create',
        arguments=[
            '-name', 'twobot',
            '-topic', 'robot_description'
        ],
        output='screen'
    )

    # Los spawners pedirán los controladores al manager que está DENTRO de Gazebo
    load_joint_state_broadcaster = Node(
        package='controller_manager',
        executable='spawner',
        arguments=['joint_state_broadcaster'],
        output='screen'
    )

    load_arm_controller = Node(
        package='controller_manager',
        executable='spawner',
        arguments=['arm_controller'],
        output='screen'
    )

    # Puente para comunicar el reloj de simulación de Gazebo a ROS 2
    ros_gz_bridge = Node(
        package='ros_gz_bridge',
        executable='parameter_bridge',
        arguments=['/clock@rosgraph_msgs/msg/Clock[gz.msgs.Clock'],
        output='screen'
    )

    return LaunchDescription([
        env_var_resources,
        robot_state_publisher_node,
        gazebo,
        spawn_entity,
        load_joint_state_broadcaster,
        load_arm_controller,
        ros_gz_bridge
    ])