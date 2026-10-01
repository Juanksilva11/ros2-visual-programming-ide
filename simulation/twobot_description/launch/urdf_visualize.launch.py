import os
from ament_index_python.packages import get_package_share_directory
from launch import LaunchDescription
from launch.substitutions import Command
from launch_ros.actions import Node
from launch_ros.parameter_descriptions import ParameterValue

def generate_launch_description():
    # 1. Define package and file paths
    pkg_name = 'twobot_description' 
    pkg_share_dir = get_package_share_directory(pkg_name)
    
    urdf_file_path = os.path.join(pkg_share_dir, 'urdf', 'twobot_v1.urdf')
    rviz_config_path = os.path.join(pkg_share_dir, 'rviz', 'twobot_config.rviz')

    # 2. Process the URDF file
    # ParameterValue ensures the output of 'cat' is correctly interpreted as a string in ROS 2 Jazzy
    robot_desc_content = Command(['cat ', urdf_file_path])
    robot_description_param = {'robot_description': ParameterValue(robot_desc_content, value_type=str)}

    # 3. Define the nodes
    robot_state_publisher_node = Node(
        package='robot_state_publisher',
        executable='robot_state_publisher',
        name='robot_state_publisher',
        output='screen',
        parameters=[robot_description_param]
    )

    joint_state_publisher_gui_node = Node(
        package='joint_state_publisher_gui',
        executable='joint_state_publisher_gui',
        name='joint_state_publisher_gui',
        output='screen'
    )

    rviz2_node = Node(
        package='rviz2',
        executable='rviz2',
        name='rviz2',
        output='screen',
        arguments=['-d', rviz_config_path]
    )

    # 4. Return the LaunchDescription
    return LaunchDescription([
        robot_state_publisher_node,
        joint_state_publisher_gui_node,
        rviz2_node
    ])
