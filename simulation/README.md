# 2-DOF manipulator simulation

ROS 2 Jazzy + Gazebo (gz sim) simulation of the two-degree-of-freedom
manipulator of UMNG research project **IMP ING 4309**: elbow
flexion/extension and wrist pronation/supination. It is the platform used
to validate the joint-position mode of the HAL (`two_dof_manipulator`
profile) and the arm scenarios S6–S7 of the measurement campaign
([`experiments/`](../experiments/)).

The model was built by the authors from the CAD files supplied by the
research group, while the physical prototype was still being
manufactured. The prototype exposes the same `ros2_control` topic
interface, so the platform profile is designed to be reused on it
unchanged — this has not yet been verified on hardware.

## Packages

| Package | Contents |
|---|---|
| `twobot_description` | URDF (`urdf/twobot_v1.urdf`): kinematic chain, joint limits, inertial parameters from the CAD mass properties, collision primitives and the `ros2_control` position interface. Fine and coarse STL meshes. RViz visualization launch. |
| `twobot_gazebo` | Empty world, spawn launch and controller configuration (`config/controllers.yaml`): `joint_state_broadcaster` + `JointTrajectoryController` at 100 Hz. |

## Interface used by the platform

| Topic | Type | Direction |
|---|---|---|
| `/arm_controller/joint_trajectory` | `trajectory_msgs/JointTrajectory` | platform → controller |
| `/joint_states` | `sensor_msgs/JointState` | simulation → platform |

| Joint | Motion | Range [rad] | Max. velocity [rad/s] |
|---|---|---|---|
| `elbow_joint` | Flexion/extension | [0, 2.44] | 2.0 |
| `wrist_joint` | Pronation/supination | [−3.14, 3.14] | 5.0 |

These limits are mirrored in the HAL profile
(`rust_app/src/hal/two_dof_manipulator.rs`) and enforced before any
motion is commanded. The wrist uses the literal URDF value 3.14 rather
than π, because the `ros2_control` command interface saturates exactly
there.

## Build

Requires ROS 2 Jazzy on Ubuntu 24.04. Build the packages in a regular
colcon workspace; a symbolic link avoids duplicating the files:

```bash
source /opt/ros/jazzy/setup.bash
mkdir -p ~/ros2_ws/src
ln -s <path-to-this-repository>/simulation ~/ros2_ws/src/twobot

# Runtime dependencies (Gazebo bridge, gz_ros2_control, controllers, RViz…)
cd ~/ros2_ws
sudo rosdep init   # first time on the machine only
rosdep update
rosdep install --from-paths src --ignore-src -y

colcon build
source install/setup.bash
```

Cloning the whole repository into `~/ros2_ws/src` also works: the other
top-level folders contain a `COLCON_IGNORE` file, so colcon only picks up
these two packages.

## Run

```bash
# Gazebo simulation with both controllers loaded (used by the platform)
ros2 launch twobot_gazebo start_world.launch.py

# Model inspection only: RViz + joint sliders, no physics
ros2 launch twobot_description urdf_visualize.launch.py
```

To drive the arm from the platform, start the simulation, run the backend
(`cd rust_app && cargo run`) and the frontend, and select the
**2DOF Manipulator** profile in the header. The palette then offers the
*Joint Move* block.

A quick check without the platform:

```bash
ros2 topic echo /joint_states --once
ros2 topic pub --once /arm_controller/joint_trajectory trajectory_msgs/msg/JointTrajectory \
  "{joint_names: [elbow_joint, wrist_joint], points: [{positions: [1.0, 0.5], time_from_start: {sec: 2}}]}"
```
