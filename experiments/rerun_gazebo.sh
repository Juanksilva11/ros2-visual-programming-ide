#!/bin/bash
# ============================================================
#  Re-ejecución de las campañas de Gazebo (Waffle + brazo 2GDL)
#  con el runner endurecido (calentamiento 6 s + reintento si el
#  bag no capturó exec_request). Recupera solo las corridas rotas.
#
#  Diseñado para correr desatendido (nohup); sobrevive al cierre
#  de la sesión. Registra todo en rerun.log.
# ============================================================
cd "$(dirname "$0")"
export TERM=dumb

set +u  # los setup.bash de ROS referencian variables no definidas
source /opt/ros/jazzy/setup.bash
source ~/Documents/Tesis/ros2_ws/install/setup.bash 2>/dev/null

PY=./.venv/bin/python
stamp() { date +%H:%M:%S; }
log() { echo "[$(stamp)] $*"; }

wait_topic() {   # $1 = tópico, $2 = timeout s
  local topic=$1 timeout=${2:-90} t=0
  while ! ros2 topic list 2>/dev/null | grep -q "^${topic}$"; do
    sleep 2; t=$((t+2))
    if [ $t -ge $timeout ]; then log "TIMEOUT esperando $topic"; return 1; fi
  done
  # esperar a que realmente publique
  timeout 15 ros2 topic echo "$topic" --once >/dev/null 2>&1
  return 0
}

kill_sim() {
  pkill -f "empty_world"        2>/dev/null
  pkill -f "start_world"        2>/dev/null
  pkill -f "ros2 launch"        2>/dev/null
  sleep 3
  pkill -9 -f "gz sim"          2>/dev/null
  pkill -9 -f "robot_state_publisher" 2>/dev/null
  pkill -9 -f "parameter_bridge"      2>/dev/null
  pkill -9 -f "spawner"               2>/dev/null
  pkill -9 -f "ros_gz"                2>/dev/null
  sleep 3
}

log "===== INICIO RE-EJECUCIÓN GAZEBO ====="

# --- Limpiar estado previo ---
kill_sim
pkill -9 -f "target/debug/rust_app" 2>/dev/null
sleep 2

# --- Backend ---
log "Iniciando backend..."
( cd ../rust_app && exec ./target/debug/rust_app ) > backend_rerun.log 2>&1 &
BACKEND_PID=$!
sleep 8
if ! curl -s http://localhost:3000/api/profiles >/dev/null; then
  log "ERROR: backend no responde"; exit 1
fi
log "Backend listo (pid $BACKEND_PID)"

# ============================================================
#  CAMPAÑA 1 — TurtleBot3 Waffle (Gazebo)
# ============================================================
log "--- Lanzando Gazebo Waffle ---"
export TURTLEBOT3_MODEL=waffle
( exec ros2 launch turtlebot3_gazebo empty_world.launch.py ) > sim_waffle.log 2>&1 &
if wait_topic "/odom" 120; then
  log "Waffle listo. Ejecutando escenarios..."
  for S in S1_min S2_line_return S3_square S4_long S5_estop; do
    log "== waffle $S =="
    $PY runner.py $S --platform waffle -n 30 --out data
  done
  log "Campaña Waffle terminada."
else
  log "ERROR: Waffle no arrancó; se omite."
fi
kill_sim

# ============================================================
#  CAMPAÑA 2 — Manipulador 2GDL (Gazebo)
# ============================================================
log "--- Lanzando Gazebo manipulador ---"
( exec ros2 launch twobot_gazebo start_world.launch.py ) > sim_arm.log 2>&1 &
if wait_topic "/joint_states" 120; then
  log "Manipulador listo. Ejecutando escenarios..."
  for S in S6_arm_cycle S7_arm_estop; do
    log "== arm $S =="
    $PY runner.py $S --platform arm -n 30 --out data
  done
  log "Campaña manipulador terminada."
else
  log "ERROR: manipulador no arrancó; se omite."
fi
kill_sim

# --- Cerrar backend ---
kill $BACKEND_PID 2>/dev/null
pkill -9 -f "target/debug/rust_app" 2>/dev/null

log "===== RE-EJECUCIÓN COMPLETA ====="
log "Verificación de integridad:"
$PY diagnose.py 2>&1 | tail -18
touch RERUN_DONE
log "FIN"
