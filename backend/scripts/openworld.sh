#!/usr/bin/env bash
# ─────────────────────────────────────────────────────────────────────────────
# openworld.sh — OpenWorld Agentic Travel CLI
#
# USAGE
#   ./scripts/openworld.sh <command> [args]
#
# COMMANDS
#   plan <destination> <days> <budget>   Full end-to-end trip planning + booking
#   status   <session_id>                Show current session state + spending
#   logs     <session_id>                Print all agent activity logs with colour
#   report   <session_id>                Print the Markdown travel report
#   watch    <session_id>                Poll logs live until session completes
#   health                               Check if the API server is reachable
#
# ENVIRONMENT
#   OPENWORLD_URL   API base URL (default: http://localhost:3000)
#
# EXAMPLES
#   ./scripts/openworld.sh plan "Tokyo" 5 1200
#   ./scripts/openworld.sh plan "Bangkok to Paris" 7 2000
#   ./scripts/openworld.sh status a1b2c3d4-...
#   ./scripts/openworld.sh report a1b2c3d4-...
#   ./scripts/openworld.sh health
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

BASE_URL="${OPENWORLD_URL:-http://localhost:3000}"

# ── Colours ──────────────────────────────────────────────────────────────────
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
CYAN='\033[0;36m'
MAGENTA='\033[0;35m'
BOLD='\033[1m'
DIM='\033[2m'
NC='\033[0m'

# ── Helpers ───────────────────────────────────────────────────────────────────

check_deps() {
  local missing=()
  for cmd in curl jq; do
    command -v "$cmd" &>/dev/null || missing+=("$cmd")
  done
  if [[ ${#missing[@]} -gt 0 ]]; then
    echo -e "${RED}Missing dependencies: ${missing[*]}${NC}"
    echo -e "${DIM}Install with: brew install ${missing[*]}${NC}"
    exit 1
  fi
}

check_server() {
  if ! curl -sf "$BASE_URL/health" &>/dev/null; then
    echo -e "${RED}✗ Server not reachable at ${BASE_URL}${NC}"
    echo -e "${DIM}  Start it first:  cargo run --bin api${NC}"
    exit 1
  fi
}

log_icon() {
  case "$1" in
    info)    echo -e "${BLUE}ℹ${NC}" ;;
    success) echo -e "${GREEN}✓${NC}" ;;
    warning) echo -e "${YELLOW}⚠${NC}" ;;
    error)   echo -e "${RED}✗${NC}" ;;
    action)  echo -e "${CYAN}▶${NC}" ;;
    *)       echo -e "·" ;;
  esac
}

print_log_entry() {
  local ts agent msg type
  ts=$(echo "$1"   | jq -r '.timestamp')
  agent=$(echo "$1" | jq -r '.agent')
  msg=$(echo "$1"   | jq -r '.message')
  type=$(echo "$1"  | jq -r '.log_type')

  local icon
  icon=$(log_icon "$type")

  local agent_color
  case "$agent" in
    PlannerAgent)     agent_color="${MAGENTA}" ;;
    SearchAgent)      agent_color="${CYAN}" ;;
    ReservationAgent) agent_color="${YELLOW}" ;;
    RecoveryAgent)    agent_color="${RED}" ;;
    VaultAgent)       agent_color="${GREEN}" ;;
    ArtifactAgent)    agent_color="${BLUE}" ;;
    *)                agent_color="${DIM}" ;;
  esac

  printf "  ${DIM}%s${NC} %s ${agent_color}%-18s${NC}  %s\n" \
    "$ts" "$icon" "$agent" "$msg"
}

# ── Commands ──────────────────────────────────────────────────────────────────

cmd_health() {
  check_deps
  echo -ne "Checking ${BASE_URL}/health ... "
  local resp
  resp=$(curl -sf "$BASE_URL/health") || {
    echo -e "${RED}OFFLINE${NC}"
    exit 1
  }
  local version
  version=$(echo "$resp" | jq -r '.version // "unknown"')
  echo -e "${GREEN}OK${NC}  (v${version})"
}

cmd_plan() {
  check_deps
  check_server

  local destination="${1:-Tokyo}"
  local days="${2:-5}"
  local budget="${3:-1200}"

  echo ""
  echo -e "${BOLD}╔══════════════════════════════════════════════════╗${NC}"
  echo -e "${BOLD}║       🌏  OpenWorld Trip Planner                 ║${NC}"
  echo -e "${BOLD}╚══════════════════════════════════════════════════╝${NC}"
  echo ""
  echo -e "  ${CYAN}Destination :${NC} ${BOLD}${destination}${NC}"
  echo -e "  ${CYAN}Duration    :${NC} ${days} days"
  echo -e "  ${CYAN}Budget      :${NC} \$${budget} USD"
  echo ""

  # Build travel.md YAML inline
  local travel_md
  travel_md=$(cat <<YAML
trip:
  destination: ${destination}
  duration_days: ${days}
  budget_max: "${budget} USD"
flight:
  max_stops: 1
  avoid_red_eye: true
  preferred_airlines: [ANA, JAL, Emirates, Singapore Airlines]
hotel:
  min_rating: 4.0
  max_price_per_night: "$(( budget / days / 3 )) USD"
  near_station: true
transport:
  prefer_train: true
  avoid_overnight_bus: true
automation:
  auto_reserve: true
  retry_on_failure: true
  allow_replanning: true
  max_retries: 3
vault:
  auto_payment: true
  max_single_transaction: "$(( budget / 3 )) USD"
YAML
)

  # Step 1 — Create session
  echo -e "${DIM}[1/3] Creating session...${NC}"
  local create_resp
  create_resp=$(curl -sf -X POST "$BASE_URL/sessions" \
    -H "Content-Type: application/json" \
    -d "$(jq -n --arg md "$travel_md" '{travel_md: $md}')") || {
    echo -e "${RED}Failed to create session${NC}"
    exit 1
  }

  local session_id
  session_id=$(echo "$create_resp" | jq -r '.session_id')
  echo -e "  ${GREEN}✓${NC} Session created: ${BOLD}${session_id}${NC}"
  echo ""

  # Step 2 — Start orchestration
  echo -e "${DIM}[2/3] Launching AI agents...${NC}"
  curl -sf -X POST "$BASE_URL/sessions/${session_id}/start" \
    -H "Content-Type: application/json" \
    -d '{}' &>/dev/null || {
    echo -e "${RED}Failed to start session${NC}"
    exit 1
  }
  echo -e "  ${GREEN}✓${NC} Orchestration started"
  echo ""

  # Step 3 — Watch until complete
  echo -e "${DIM}[3/3] Watching agent pipeline...${NC}"
  echo -e "${DIM}      (Ctrl+C to detach — session keeps running)${NC}"
  echo ""

  cmd_watch "$session_id"
}

cmd_watch() {
  check_deps
  check_server

  local session_id="$1"
  local last_count=0
  local state="running"
  local spinner=('⠋' '⠙' '⠹' '⠸' '⠼' '⠴' '⠦' '⠧' '⠇' '⠏')
  local spin_i=0

  echo -e "  ${DIM}Session: ${session_id}${NC}"
  echo ""

  while [[ "$state" != "complete" && "$state" != "failed" ]]; do
    local detail
    detail=$(curl -sf "$BASE_URL/sessions/${session_id}" 2>/dev/null) || {
      sleep 2; continue
    }

    state=$(echo "$detail" | jq -r '.state')
    local log_count
    log_count=$(echo "$detail" | jq -r '.log_count')

    # Print any new log lines
    if [[ "$log_count" -gt "$last_count" ]]; then
      local logs
      logs=$(curl -sf "$BASE_URL/sessions/${session_id}/logs" 2>/dev/null) || true

      local new_entries
      new_entries=$(echo "$logs" | jq -c ".[$last_count:][]" 2>/dev/null) || true

      while IFS= read -r entry; do
        [[ -z "$entry" ]] && continue
        print_log_entry "$entry"
      done <<< "$new_entries"

      last_count="$log_count"
    else
      # Show spinner while waiting
      printf "\r  ${DIM}${spinner[$spin_i]} Waiting for agents (%s)...${NC}  " "$state"
      spin_i=$(( (spin_i + 1) % ${#spinner[@]} ))
    fi

    if [[ "$state" == "complete" || "$state" == "failed" ]]; then
      break
    fi

    sleep 2
  done

  printf "\r%80s\r" ""  # Clear spinner line

  echo ""
  if [[ "$state" == "complete" ]]; then
    echo -e "  ${GREEN}${BOLD}✓ Session complete!${NC}"
    echo ""
    cmd_summary "$session_id"
  else
    echo -e "  ${RED}${BOLD}✗ Session failed${NC}"
    cmd_status "$session_id"
    exit 1
  fi
}

cmd_status() {
  check_deps
  check_server

  local session_id="$1"
  local detail
  detail=$(curl -sf "$BASE_URL/sessions/${session_id}") || {
    echo -e "${RED}Session not found: ${session_id}${NC}"
    exit 1
  }

  local state dest budget duration log_count
  state=$(echo "$detail"     | jq -r '.state')
  dest=$(echo "$detail"      | jq -r '.policy.trip.destination // "unknown"')
  budget=$(echo "$detail"    | jq -r '.policy.trip.budget_max // 0')
  duration=$(echo "$detail"  | jq -r '.policy.trip.duration_days // 0')
  log_count=$(echo "$detail" | jq -r '.log_count')

  local state_color
  case "$state" in
    complete) state_color="${GREEN}" ;;
    failed)   state_color="${RED}" ;;
    *)        state_color="${YELLOW}" ;;
  esac

  echo ""
  echo -e "  Session    ${BOLD}${session_id}${NC}"
  echo -e "  State      ${state_color}${BOLD}${state}${NC}"
  echo -e "  Trip       ${dest} — ${duration} days"
  echo -e "  Budget     \$${budget} USD"
  echo -e "  Log lines  ${log_count}"
  echo ""
}

cmd_summary() {
  local session_id="$1"
  local artifact
  artifact=$(curl -sf "$BASE_URL/sessions/${session_id}/artifact" 2>/dev/null) || return

  local spent dest duration bookings
  spent=$(echo "$artifact"    | jq -r '.total_spent_usd // 0')
  dest=$(echo "$artifact"     | jq -r '.destination // "unknown"')
  duration=$(echo "$artifact" | jq -r '.duration_days // 0')
  bookings=$(echo "$artifact" | jq '.bookings | length // 0')

  echo -e "  ${BOLD}Trip Summary${NC}"
  echo -e "  ─────────────────────────────────"
  echo -e "  Destination  ${BOLD}${dest}${NC}"
  echo -e "  Duration     ${duration} days"
  echo -e "  Total Spent  ${GREEN}${BOLD}\$${spent} USD${NC}"
  echo -e "  Bookings     ${bookings} confirmed"
  echo ""
  echo -e "  To see the full report:"
  echo -e "  ${DIM}./scripts/openworld.sh report ${session_id}${NC}"
  echo ""
}

cmd_logs() {
  check_deps
  check_server

  local session_id="$1"
  local logs
  logs=$(curl -sf "$BASE_URL/sessions/${session_id}/logs") || {
    echo -e "${RED}Session not found: ${session_id}${NC}"
    exit 1
  }

  echo ""
  echo -e "  ${BOLD}Agent Activity Log${NC}  ${DIM}(${session_id})${NC}"
  echo -e "  ─────────────────────────────────────────────────────"
  echo ""

  local count=0
  while IFS= read -r entry; do
    [[ -z "$entry" ]] && continue
    print_log_entry "$entry"
    (( count++ )) || true
  done < <(echo "$logs" | jq -c '.[]')

  echo ""
  echo -e "  ${DIM}${count} entries${NC}"
  echo ""
}

cmd_report() {
  check_deps
  check_server

  local session_id="$1"
  local report
  report=$(curl -sf "$BASE_URL/sessions/${session_id}/report") || {
    echo -e "${RED}Report not available for session: ${session_id}${NC}"
    echo -e "${DIM}Session may still be running, or report generation failed.${NC}"
    exit 1
  }

  echo ""
  echo "$report"
  echo ""
}

usage() {
  echo ""
  echo -e "${BOLD}OpenWorld Agentic Travel CLI${NC}"
  echo ""
  echo -e "  ${CYAN}USAGE${NC}"
  echo "    ./scripts/openworld.sh <command> [args]"
  echo ""
  echo -e "  ${CYAN}COMMANDS${NC}"
  echo "    plan <destination> <days> <budget>   Plan and book a trip end-to-end"
  echo "    status   <session_id>                Show session state and trip info"
  echo "    logs     <session_id>                Print all agent activity logs"
  echo "    report   <session_id>                Print the Markdown travel report"
  echo "    watch    <session_id>                Live-poll logs until complete"
  echo "    health                               Check if API server is running"
  echo ""
  echo -e "  ${CYAN}EXAMPLES${NC}"
  echo "    ./scripts/openworld.sh plan \"Tokyo\" 5 1200"
  echo "    ./scripts/openworld.sh plan \"Paris\" 7 2500"
  echo "    ./scripts/openworld.sh plan \"Bangkok to Osaka\" 10 1800"
  echo ""
  echo -e "  ${CYAN}ENVIRONMENT${NC}"
  echo "    OPENWORLD_URL   Override API base URL (default: http://localhost:3000)"
  echo ""
  echo -e "  ${DIM}Start the server first:  cargo run --bin api${NC}"
  echo ""
}

# ── Entry Point ───────────────────────────────────────────────────────────────

CMD="${1:-}"
shift || true

case "$CMD" in
  plan)
    DEST="${1:-Tokyo}"
    DAYS="${2:-5}"
    BUDGET="${3:-1200}"
    cmd_plan "$DEST" "$DAYS" "$BUDGET"
    ;;
  status)
    [[ -z "${1:-}" ]] && { echo -e "${RED}Usage: $0 status <session_id>${NC}"; exit 1; }
    cmd_status "$1"
    ;;
  logs)
    [[ -z "${1:-}" ]] && { echo -e "${RED}Usage: $0 logs <session_id>${NC}"; exit 1; }
    cmd_logs "$1"
    ;;
  report)
    [[ -z "${1:-}" ]] && { echo -e "${RED}Usage: $0 report <session_id>${NC}"; exit 1; }
    cmd_report "$1"
    ;;
  watch)
    [[ -z "${1:-}" ]] && { echo -e "${RED}Usage: $0 watch <session_id>${NC}"; exit 1; }
    cmd_watch "$1"
    ;;
  health)
    cmd_health
    ;;
  *)
    usage
    ;;
esac
