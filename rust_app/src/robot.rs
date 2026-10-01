use futures::StreamExt;
use r2r::Publisher;
use r2r::builtin_interfaces::msg::Duration as RosDuration;
use r2r::geometry_msgs::msg::{Twist, TwistStamped};
use r2r::sensor_msgs::msg::JointState;
use r2r::trajectory_msgs::msg::{JointTrajectory, JointTrajectoryPoint};
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};

use crate::hal::{ControlMode, OdomKind, OdomState, RobotProfile, VelocityKind};

// ─── Velocity Publisher Abstraction ─────────────────────────────────────────

/// Wraps either a Twist or TwistStamped publisher behind a unified interface.
#[derive(Clone)]
enum VelocityPublisher {
    Twist(Publisher<Twist>),
    TwistStamped(Publisher<TwistStamped>),
}

impl VelocityPublisher {
    fn publish(&self, linear_x: f64, angular_z: f64) {
        match self {
            VelocityPublisher::Twist(pub_) => {
                let mut msg = Twist::default();
                msg.linear.x = linear_x;
                msg.angular.z = angular_z;
                let _ = pub_.publish(&msg);
            }
            VelocityPublisher::TwistStamped(pub_) => {
                let mut msg = TwistStamped::default();
                msg.twist.linear.x = linear_x;
                msg.twist.angular.z = angular_z;
                // header.stamp is left at zero — the receiving node will use its own clock
                let _ = pub_.publish(&msg);
            }
        }
    }

    fn stop(&self) {
        self.publish(0.0, 0.0);
    }
}

// ─── Robot Context ──────────────────────────────────────────────────────────

/// Thread-safe, profile-aware robot context.
#[derive(Clone)]
pub struct RobotContext {
    profile: Arc<RobotProfile>,
    publishers: Arc<Mutex<HashMap<String, VelocityPublisher>>>,
    joint_publisher: Arc<Mutex<Option<Publisher<JointTrajectory>>>>,
    node: Arc<Mutex<r2r::Node>>,
    latest_odom: Arc<Mutex<Option<OdomState>>>,
    /// Latest joint positions, ordered like `profile.joints`
    latest_joints: Arc<Mutex<Option<Vec<f64>>>>,
}

impl RobotContext {
    /// Create a new RobotContext bound to the given profile.
    /// Automatically subscribes to the profile's odometry topic.
    pub fn new(node: Arc<Mutex<r2r::Node>>, profile: RobotProfile) -> Self {
        let ctx = Self {
            profile: Arc::new(profile.clone()),
            publishers: Arc::new(Mutex::new(HashMap::new())),
            joint_publisher: Arc::new(Mutex::new(None)),
            node: node.clone(),
            latest_odom: Arc::new(Mutex::new(None)),
            latest_joints: Arc::new(Mutex::new(None)),
        };

        // Start the feedback listeners based on the profile
        ctx.start_odom_listener(&profile, node.clone());
        ctx.start_joint_state_listener(&profile, node);

        ctx
    }

    /// Re-initialize the context with a new robot profile.
    /// Clears publishers + odom state and re-subscribes.
    pub fn switch_profile(&mut self, profile: RobotProfile, node: Arc<Mutex<r2r::Node>>) {
        tracing::info!("Switching robot profile to: {}", profile.name);

        // Clear caches
        {
            let mut pubs = self.publishers.lock().unwrap();
            pubs.clear();
        }
        {
            let mut joint_pub = self.joint_publisher.lock().unwrap();
            *joint_pub = None;
        }
        {
            let mut odom = self.latest_odom.lock().unwrap();
            *odom = None;
        }
        {
            let mut joints = self.latest_joints.lock().unwrap();
            *joints = None;
        }

        self.profile = Arc::new(profile.clone());
        self.start_odom_listener(&profile, node.clone());
        self.start_joint_state_listener(&profile, node);
    }

    fn start_odom_listener(&self, profile: &RobotProfile, node: Arc<Mutex<r2r::Node>>) {
        let odom_mem = self.latest_odom.clone();

        match (&profile.odom_type, &profile.odom_topic) {
            (OdomKind::TurtlesimPose, Some(topic)) => {
                let mut sub = {
                    let mut n = node.lock().unwrap();
                    n.subscribe::<r2r::turtlesim::msg::Pose>(topic, r2r::QosProfile::default())
                        .expect("Failed to subscribe to turtlesim pose")
                };
                tokio::spawn(async move {
                    while let Some(msg) = sub.next().await {
                        let state = OdomState {
                            x: msg.x as f64,
                            y: msg.y as f64,
                            yaw: msg.theta as f64,
                        };
                        let mut guard = odom_mem.lock().unwrap();
                        *guard = Some(state);
                    }
                });
            }
            (OdomKind::NavOdometry, Some(topic)) => {
                let mut sub = {
                    let mut n = node.lock().unwrap();
                    n.subscribe::<r2r::nav_msgs::msg::Odometry>(topic, r2r::QosProfile::default())
                        .expect("Failed to subscribe to nav_msgs/Odometry")
                };
                tokio::spawn(async move {
                    while let Some(msg) = sub.next().await {
                        // Extract yaw from quaternion (z-axis rotation)
                        let q = &msg.pose.pose.orientation;
                        let siny_cosp = 2.0 * (q.w * q.z + q.x * q.y);
                        let cosy_cosp = 1.0 - 2.0 * (q.y * q.y + q.z * q.z);
                        let yaw = siny_cosp.atan2(cosy_cosp);

                        let state = OdomState {
                            x: msg.pose.pose.position.x,
                            y: msg.pose.pose.position.y,
                            yaw,
                        };
                        let mut guard = odom_mem.lock().unwrap();
                        *guard = Some(state);
                    }
                });
            }
            (OdomKind::None, _) | (_, None) => {
                tracing::warn!("No odometry configured for profile: {}", profile.name);
            }
        }
    }

    /// Subscribes to the profile's joint_state topic (manipulators only)
    /// and stores the latest positions **ordered like `profile.joints`**.
    /// `sensor_msgs/JointState` gives no ordering guarantee, so each
    /// message is remapped by joint name.
    fn start_joint_state_listener(&self, profile: &RobotProfile, node: Arc<Mutex<r2r::Node>>) {
        if profile.control_mode != ControlMode::JointPosition {
            return;
        }
        let Some(topic) = &profile.joint_state_topic else {
            tracing::warn!("No joint_state topic configured for profile: {}", profile.name);
            return;
        };

        let mut sub = {
            let mut n = node.lock().unwrap();
            n.subscribe::<JointState>(topic, r2r::QosProfile::default())
                .expect("Failed to subscribe to JointState")
        };

        let joint_names: Vec<String> = profile.joints.iter().map(|j| j.name.clone()).collect();
        let joints_mem = self.latest_joints.clone();

        tokio::spawn(async move {
            while let Some(msg) = sub.next().await {
                let mapped: Option<Vec<f64>> = joint_names
                    .iter()
                    .map(|name| {
                        msg.name
                            .iter()
                            .position(|n| n == name)
                            .and_then(|idx| msg.position.get(idx).copied())
                    })
                    .collect();
                if let Some(positions) = mapped {
                    let mut guard = joints_mem.lock().unwrap();
                    *guard = Some(positions);
                }
            }
        });
    }

    // ─── Public API ─────────────────────────────────────────────────────

    /// Publish a velocity command using the correct message type for the active robot.
    pub fn publish_velocity(&mut self, linear_x: f64, angular_z: f64) -> Result<(), String> {
        let topic = self.profile.cmd_vel_topic.clone();
        let publisher = self.get_or_create_publisher(&topic)?;
        publisher.publish(linear_x, angular_z);
        Ok(())
    }

    /// Command a joint-space motion: one JointTrajectory with a single
    /// point at `positions` (ordered like `profile.joints`), interpolated
    /// by the controller over `duration_ms`.
    pub fn publish_joint_trajectory(
        &mut self,
        positions: Vec<f64>,
        duration_ms: u64,
    ) -> Result<(), String> {
        let publisher = self.get_or_create_joint_publisher()?;

        let point = JointTrajectoryPoint {
            positions,
            time_from_start: RosDuration {
                sec: (duration_ms / 1000) as i32,
                nanosec: ((duration_ms % 1000) * 1_000_000) as u32,
            },
            ..Default::default()
        };
        let msg = JointTrajectory {
            joint_names: self.profile.joints.iter().map(|j| j.name.clone()).collect(),
            points: vec![point],
            ..Default::default()
        };
        publisher
            .publish(&msg)
            .map_err(|e| format!("Failed to publish joint trajectory: {}", e))?;
        Ok(())
    }

    /// Latest joint positions, ordered like `profile.joints`.
    pub fn get_current_joints(&self) -> Option<Vec<f64>> {
        let guard = self.latest_joints.lock().unwrap();
        guard.clone()
    }

    /// Ensures the joint trajectory publisher exists and has at least one
    /// matched subscriber (the controller), waiting up to `timeout`.
    ///
    /// JointTrajectory commands are ONE-SHOT messages: publishing before
    /// DDS discovery completes silently loses the command (unlike cmd_vel,
    /// which is re-published continuously). Called at profile selection to
    /// warm up discovery, and defensively before every joint motion.
    pub async fn wait_for_joint_subscribers(&mut self, timeout: std::time::Duration) -> bool {
        let Ok(publisher) = self.get_or_create_joint_publisher() else {
            return false;
        };
        let deadline = std::time::Instant::now() + timeout;
        loop {
            if publisher.get_inter_process_subscription_count().unwrap_or(0) > 0 {
                return true;
            }
            if std::time::Instant::now() >= deadline {
                return false;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        }
    }

    /// Emergency/normal stop, dispatched by control mode:
    /// - Velocity robots: publish a zero-velocity command.
    /// - Manipulators: publish an EMPTY JointTrajectory, which cancels the
    ///   controller's active trajectory and holds the current position
    ///   (zero velocity is meaningless for a position-controlled arm).
    pub fn stop(&mut self) -> Result<(), String> {
        match self.profile.control_mode {
            ControlMode::Velocity => {
                let topic = self.profile.cmd_vel_topic.clone();
                let publisher = self.get_or_create_publisher(&topic)?;
                publisher.stop();
                Ok(())
            }
            ControlMode::JointPosition => {
                let publisher = self.get_or_create_joint_publisher()?;
                // This JointTrajectoryController version REJECTS empty
                // trajectories ("Empty trajectory received"), so the
                // cancel-by-empty-message convention is unavailable.
                // Instead we exploit guaranteed topic-interface trajectory
                // REPLACEMENT: command a single point at the CURRENT
                // position with a short duration — "go to where you
                // already are" halts the arm and holds it there.
                let current = self
                    .get_current_joints()
                    .ok_or_else(|| "Cannot stop arm: no joint state received yet".to_string())?;
                let point = JointTrajectoryPoint {
                    positions: current,
                    time_from_start: RosDuration {
                        sec: 0,
                        nanosec: 200_000_000, // 200 ms
                    },
                    ..Default::default()
                };
                let msg = JointTrajectory {
                    joint_names: self.profile.joints.iter().map(|j| j.name.clone()).collect(),
                    points: vec![point],
                    ..Default::default()
                };
                publisher
                    .publish(&msg)
                    .map_err(|e| format!("Failed to publish stop trajectory: {}", e))?;
                Ok(())
            }
        }
    }

    /// Get the latest odometry state (robot-agnostic).
    pub fn get_current_odom(&self) -> Option<OdomState> {
        let guard = self.latest_odom.lock().unwrap();
        guard.clone()
    }

    /// Get a reference to the active robot profile.
    pub fn profile(&self) -> &RobotProfile {
        &self.profile
    }

    pub fn clone_ref(&self) -> Self {
        self.clone()
    }

    // ─── Internal ───────────────────────────────────────────────────────

    fn get_or_create_joint_publisher(&mut self) -> Result<Publisher<JointTrajectory>, String> {
        {
            let guard = self.joint_publisher.lock().unwrap();
            if let Some(p) = guard.as_ref() {
                return Ok(p.clone());
            }
        }

        let topic = self
            .profile
            .joint_command_topic
            .clone()
            .ok_or_else(|| format!("Profile {} has no joint command topic", self.profile.name))?;

        let publisher = {
            let mut node_guard = self.node.lock().unwrap();
            node_guard
                .create_publisher::<JointTrajectory>(&topic, r2r::QosProfile::default())
                .map_err(|e| format!("Failed to create JointTrajectory publisher: {}", e))?
        };
        tracing::info!("Created JointTrajectory publisher for: {}", topic);

        let mut guard = self.joint_publisher.lock().unwrap();
        *guard = Some(publisher.clone());
        Ok(publisher)
    }

    fn get_or_create_publisher(&mut self, topic: &str) -> Result<VelocityPublisher, String> {
        let mut pubs = self.publishers.lock().unwrap();
        if let Some(p) = pubs.get(topic) {
            return Ok(p.clone());
        }

        let mut node_guard = self.node.lock().unwrap();
        let publisher = match self.profile.cmd_vel_type {
            VelocityKind::Twist => {
                let p = node_guard
                    .create_publisher::<Twist>(topic, r2r::QosProfile::default())
                    .map_err(|e| format!("Failed to create Twist publisher: {}", e))?;
                tracing::info!("Created Twist publisher for: {}", topic);
                VelocityPublisher::Twist(p)
            }
            VelocityKind::TwistStamped => {
                let p = node_guard
                    .create_publisher::<TwistStamped>(topic, r2r::QosProfile::default())
                    .map_err(|e| format!("Failed to create TwistStamped publisher: {}", e))?;
                tracing::info!("Created TwistStamped publisher for: {}", topic);
                VelocityPublisher::TwistStamped(p)
            }
        };

        pubs.insert(topic.to_string(), publisher.clone());
        Ok(publisher)
    }
}
