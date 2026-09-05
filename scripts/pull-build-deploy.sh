#!/usr/bin/env bash
set -euo pipefail

trap '' HUP PIPE

if [[ -n "${BASH_SOURCE[0]:-}" && "${BASH_SOURCE[0]}" != "bash" ]]; then
  REPO_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
  SCRIPT_NAME="$(basename "${BASH_SOURCE[0]}")"
else
  REPO_DIR="/opt/groupscape-web"
  SCRIPT_NAME="pull-build-deploy.sh"
fi
LOCK_DIR="/tmp/groupscape_web_deploy.lock"
export COMPOSE_PROJECT_NAME="groupscape-web"

CHECK_INTERVAL=30
BRANCH="main"
COMPOSE_FILE="docker-compose.prod.yml"
COMPOSE_ARGS=(-f "$COMPOSE_FILE")

# Containers that must be healthy for a deploy to count as successful.
HEALTH_SERVICES=(groupscape-web-backend groupscape-web-frontend groupscape-web-db)
# Application images we build, tag, roll back, and prune (db is an upstream image).
APP_IMAGES=(groupscape-web-backend groupscape-web-frontend)

# Auto-retry a failing deploy this many times before waiting for a new commit.
MAX_DEPLOY_RETRIES=3
FAILED_HASH=""
FAILED_COUNT=0

FORCE_DEPLOY=false
RUN_ONCE=false

while [[ $# -gt 0 ]]; do
  case "$1" in
    --force)       FORCE_DEPLOY=true ;;
    --once)        RUN_ONCE=true ;;
    --branch=*)    BRANCH="${1#--branch=}" ;;
    --work-dir=*)  REPO_DIR="${1#--work-dir=}" ;;
  esac
  shift
done

LOG_ROOT="$REPO_DIR/logs"
DEPLOY_LOG_DIR="$LOG_ROOT/deploy"
DOCKER_LOG_DIR="$LOG_ROOT/docker"
GIT_LOG_DIR="$LOG_ROOT/git"
mkdir -p "$DEPLOY_LOG_DIR" "$DOCKER_LOG_DIR" "$GIT_LOG_DIR"

DATE_STR="$(date '+%Y-%m-%d')"
DEPLOY_LOG_FILE="$DEPLOY_LOG_DIR/deploy-$DATE_STR.log"
DOCKER_LOG_FILE="$DOCKER_LOG_DIR/docker-$DATE_STR.log"
GIT_LOG_FILE="$GIT_LOG_DIR/git-$DATE_STR.log"

# Tracks the last hash we *successfully* deployed. Lives under the gitignored
# logs/ dir, so `git reset --hard` (which only touches tracked files) never
# removes it. Decouples "what's deployed" from HEAD, so a deploy that fails after
# the working tree was synced still retries instead of being masked.
STATE_FILE="$LOG_ROOT/last-deployed-hash"

log()         { echo "[$(date '+%Y-%m-%d %H:%M:%S')] $*"; }
log_info()    { log "[INFO] $*"; }
log_warn()    { log "[WARN] $*"; }
log_error()   { log "[ERROR] $*" >&2; }
log_success() { log "[SUCCESS] $*"; }
timestamp()   { while IFS= read -r line; do echo "[$(date '+%Y-%m-%d %H:%M:%S')] $line"; done; }
tag_lines()   { local TAG="$1"; while IFS= read -r line; do echo "[$TAG] $line"; done; }

exec > >(tee -a "$DEPLOY_LOG_FILE") 2>&1
TEE_PID=$!

echo "========================================"
log_info "Starting GroupScape Web deploy watcher"
# Vantage (server/lib/deployer.js::runDeploy) scrapes this marker out of deploy
# output to auto-set deploy_configs.git_work_dir — without it, its git pre-check
# defaults to the wrong path and always reports "no changes".
echo "[VANTAGE_WORK_DIR=$REPO_DIR]"
log_info "Repo: $REPO_DIR"
log_info "Branch: $BRANCH"
log_info "Force mode: $FORCE_DEPLOY"
log_info "Run once: $RUN_ONCE"
echo "========================================"

cd "$REPO_DIR"

if [ ! -d ".git" ]; then
  log_error "Not a git repository: $REPO_DIR"
  exit 1
fi

# Load .env so PG_USER/PG_DB are available for health checks
if [ -f "$REPO_DIR/.env" ]; then
  # Strip CRLF in case .env was authored/edited on Windows — a trailing \r
  # leaks into every var and breaks numeric/hostname comparisons downstream.
  set -o allexport
  # shellcheck disable=SC1090
  source <(sed 's/\r$//' "$REPO_DIR/.env")
  set +o allexport
  log_info ".env loaded"
else
  log_warn ".env not found — PG_USER/PG_DB will default to 'postgres'/'groupscape_db' for health checks"
fi

cleanup_logs() {
  log_info "Cleaning logs older than 90 days..."
  find "$DEPLOY_LOG_DIR" -type f -name "*.log" -mtime +90 -delete
  find "$DOCKER_LOG_DIR" -type f -name "*.log" -mtime +90 -delete
  find "$GIT_LOG_DIR"    -type f -name "*.log" -mtime +90 -delete
  log_success "Log cleanup completed"
}
cleanup_logs

# On exit: log unexpected failures, release the lock, then close stdout/stderr so
# the `tee` writer sees EOF and we can wait for it to flush — otherwise the tail
# of the log can be truncated when the script exits.
trap 'STATUS=$?; [ $STATUS -ne 0 ] && log_error "Script terminated unexpectedly (exit $STATUS)"; release_lock; exec >&- 2>&- || true; [ -n "${TEE_PID:-}" ] && wait "$TEE_PID" 2>/dev/null || true' EXIT

acquire_lock() {
  # `mkdir` is atomic, so the check-and-take cannot race two concurrent runs.
  if mkdir "$LOCK_DIR" 2>/dev/null; then
    return 0
  fi
  local LOCK_AGE
  LOCK_AGE=$(( $(date +%s) - $(stat -c %Y "$LOCK_DIR" 2>/dev/null || echo 0) ))
  if [ "$LOCK_AGE" -gt 1800 ]; then
    log_warn "Stale lock detected (age: ${LOCK_AGE}s) — removing and retrying"
    rm -rf "$LOCK_DIR"
    if mkdir "$LOCK_DIR" 2>/dev/null; then
      return 0
    fi
    log_warn "Could not acquire lock after clearing stale lock. Skipping."
    return 1
  fi
  log_warn "Another deployment is running (lock age: ${LOCK_AGE}s). Skipping."
  return 1
}
release_lock() { rm -rf "$LOCK_DIR"; }

check_updates() {
  log_info "Checking for updates on branch '$BRANCH'..."
  if ! git fetch origin "$BRANCH" 2>&1 | timestamp >> "$GIT_LOG_FILE"; then
    log_error "git fetch failed — check network/SSH access"
    return 1
  fi
  REMOTE_HASH=$(git rev-parse "origin/$BRANCH")
  # Compare against what we last *deployed*, not HEAD — a deploy that fails after
  # syncing the tree leaves HEAD at the remote, so a HEAD comparison would hide
  # the still-pending work. Fall back to HEAD on first run (no state file yet).
  local DEPLOYED_HASH
  DEPLOYED_HASH=$(cat "$STATE_FILE" 2>/dev/null || git rev-parse HEAD)
  log_info "Last deployed: $DEPLOYED_HASH"
  log_info "Remote HEAD:   $REMOTE_HASH"
  if [ "$REMOTE_HASH" = "$DEPLOYED_HASH" ]; then
    log_info "No updates found — skipping deploy"
    return 1
  fi
  if [ "$REMOTE_HASH" = "$FAILED_HASH" ] && [ "$FAILED_COUNT" -ge "$MAX_DEPLOY_RETRIES" ]; then
    log_warn "Commit $REMOTE_HASH has failed ${FAILED_COUNT}× — waiting for a new commit before retrying"
    return 1
  fi
  log_success "Updates detected"
  return 0
}

# A container passes only when it is running AND its healthcheck has actually
# passed (health == "healthy"), or it has no healthcheck ("none"). "starting" and
# "unhealthy" both mean "not ready" — we keep waiting rather than declaring success.
svc_ready() {
  local STATUS="$1" HEALTH="$2"
  [ "$STATUS" = "running" ] && { [ "$HEALTH" = "healthy" ] || [ "$HEALTH" = "none" ]; }
}

check_health() {
  local TIMEOUT=180
  local ELAPSED=0
  local INTERVAL=5
  local HEARTBEAT_EVERY=15
  local LAST_HEARTBEAT=0

  log_info "Waiting for containers to be healthy (up to ${TIMEOUT}s)..."

  while [ "$ELAPSED" -lt "$TIMEOUT" ]; do
    local ALL_OK=true
    for SVC in "${HEALTH_SERVICES[@]}"; do
      STATUS=$(docker inspect --format '{{.State.Status}}' "$SVC" 2>/dev/null || echo "missing")
      HEALTH=$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$SVC" 2>/dev/null || echo "missing")
      if ! svc_ready "$STATUS" "$HEALTH"; then
        ALL_OK=false
        break
      fi
    done

    if [ "$ALL_OK" = true ]; then
      break
    fi

    if [ $(( ELAPSED - LAST_HEARTBEAT )) -ge "$HEARTBEAT_EVERY" ]; then
      log_info "Still waiting for containers to be healthy... (${ELAPSED}s/${TIMEOUT}s elapsed)"
      LAST_HEARTBEAT=$ELAPSED
    fi

    sleep "$INTERVAL"
    ELAPSED=$(( ELAPSED + INTERVAL ))
  done

  docker compose "${COMPOSE_ARGS[@]}" ps 2>&1 | tee -a "$DOCKER_LOG_FILE"

  local FAILED=false
  for SVC in "${HEALTH_SERVICES[@]}"; do
    STATUS=$(docker inspect --format '{{.State.Status}}' "$SVC" 2>/dev/null || echo "missing")
    HEALTH=$(docker inspect --format '{{if .State.Health}}{{.State.Health.Status}}{{else}}none{{end}}' "$SVC" 2>/dev/null || echo "missing")

    if [ "$STATUS" != "running" ]; then
      log_error "Container $SVC is not running (status: $STATUS)"
      FAILED=true
    elif ! svc_ready "$STATUS" "$HEALTH"; then
      log_error "Container $SVC did not become healthy in ${TIMEOUT}s (health: $HEALTH)"
      FAILED=true
    else
      log_success "$SVC — status: $STATUS, health: $HEALTH"
    fi
  done

  if [ "$FAILED" = true ]; then
    log_error "One or more containers failed — dumping logs"
    for SVC in "${HEALTH_SERVICES[@]}"; do
      log_warn "=== Logs for $SVC ==="
      docker logs --tail 50 "$SVC" 2>&1 | timestamp | tee -a "$DOCKER_LOG_FILE" || true
    done
    return 1
  fi

  return 0
}

rollback() {
  log_warn "Rolling back to previous images..."
  local ROLLBACK_OK=true
  for IMG in "${APP_IMAGES[@]}"; do
    if docker image inspect "$IMG:previous" > /dev/null 2>&1; then
      docker tag "$IMG:previous" "$IMG:latest"
      log_info "Restored $IMG:latest from $IMG:previous"
    else
      log_error "No $IMG:previous image available — cannot roll back this image"
      ROLLBACK_OK=false
    fi
  done

  if [ "$ROLLBACK_OK" = false ]; then
    log_error "Rollback incomplete — manual intervention required"
    return 1
  fi

  # --force-recreate: the running containers are on the failed :latest image ID;
  # force a recreate so they pick up the restored (previous) image.
  if ! docker compose "${COMPOSE_ARGS[@]}" up -d --force-recreate 2>&1 | timestamp >> "$DOCKER_LOG_FILE"; then
    log_error "Failed to restart containers from rolled-back images"
    return 1
  fi

  if ! check_health; then
    log_error "Containers still unhealthy after rollback — manual intervention required"
    return 1
  fi

  log_success "Rollback completed — previous images restored and healthy"
}

# Re-point the :latest tags back at the last-known-good :previous images.
# Used when we abort before recreating the app — the old containers keep running
# off their existing image, so this only ensures a stray `up` won't pick the new build.
restore_latest_tags() {
  for IMG in "${APP_IMAGES[@]}"; do
    if docker image inspect "$IMG:previous" > /dev/null 2>&1; then
      docker tag "$IMG:previous" "$IMG:latest"
      log_info "Restored $IMG:latest from $IMG:previous (deploy aborted)"
    fi
  done
}

deploy() {
  log_info "Starting deployment"
  local DEPLOY_START=$SECONDS

  # Fetch here rather than relying on check_updates() — force mode calls deploy()
  # directly, skipping check_updates, so a stale local origin/$BRANCH would
  # otherwise get redeployed on every forced run instead of the real remote HEAD.
  log_info "Fetching origin/$BRANCH..."
  if ! git fetch origin "$BRANCH" 2>&1 | timestamp >> "$GIT_LOG_FILE"; then
    log_error "git fetch failed — check network/SSH access"
    return 1
  fi

  # Diff against what's actually running (not just HEAD) so a build is never
  # skipped because a *previous* failed/forced deploy already moved HEAD past
  # the last change without rebuilding it.
  local PREV_HASH
  PREV_HASH=$(cat "$STATE_FILE" 2>/dev/null || echo "")

  # Mirror the remote rather than merge — the server never carries local commits.
  log_info "Syncing working tree to origin/$BRANCH..."
  if ! git reset --hard "origin/$BRANCH" 2>&1 | timestamp >> "$GIT_LOG_FILE"; then
    log_error "git reset --hard origin/$BRANCH failed"
    return 1
  fi
  log_success "Working tree synced to origin/$BRANCH"

  # No semver in this repo — the short commit hash doubles as the image version tag
  # (see docker image ls below, which keeps the last 2 per image).
  local COMMIT_HASH
  COMMIT_HASH=$(git rev-parse --short HEAD)
  local VERSION="$COMMIT_HASH"

  log_info "Building GroupScape Web $VERSION"

  # Skip rebuilding an image whose source directory didn't change, and whose
  # :latest tag already exists — first deploy ever (no PREV_HASH, or no existing
  # image) always builds both so the "$VERSION" tag actually exists for pruning.
  local BUILD_BACKEND=true BUILD_FRONTEND=true
  if [ -n "$PREV_HASH" ] && git cat-file -e "$PREV_HASH" 2>/dev/null; then
    if git diff --quiet "$PREV_HASH" "$COMMIT_HASH" -- server \
        && docker image inspect groupscape-web-backend:latest > /dev/null 2>&1; then
      BUILD_BACKEND=false
      log_info "server/ unchanged since $PREV_HASH — skipping backend build"
    fi
    if git diff --quiet "$PREV_HASH" "$COMMIT_HASH" -- site \
        && docker image inspect groupscape-web-frontend:latest > /dev/null 2>&1; then
      BUILD_FRONTEND=false
      log_info "site/ unchanged since $PREV_HASH — skipping frontend build"
    fi
  fi

  # Tag current images as "previous" so we can roll back if the new deploy fails health checks
  for IMG in "${APP_IMAGES[@]}"; do
    if docker image inspect "$IMG:latest" > /dev/null 2>&1; then
      docker tag "$IMG:latest" "$IMG:previous"
      log_info "Tagged $IMG:latest as $IMG:previous for rollback"
    fi
  done

  # Build backend and frontend in parallel — independent Dockerfiles/contexts.
  # Each stream is tagged and written straight to $DOCKER_LOG_FILE as it happens
  # (append writes below PIPE_BUF are atomic, so the two interleave safely) —
  # buffering a build's output until it finished previously starved Vantage's
  # log-tailing progress detection for the whole build duration.
  local BACKEND_PID="" FRONTEND_PID=""

  if [ "$BUILD_BACKEND" = true ]; then
    # Runtime config (PG_*, BACKEND_SECRET) is written to config.toml by
    # server/docker-entrypoint.sh at container start, not baked in at build
    # time — so no build args are needed here.
    ( docker build \
        -t groupscape-web-backend:latest \
        -t "groupscape-web-backend:$VERSION" \
        -f server/Dockerfile \
        server 2>&1 | tag_lines backend | timestamp >> "$DOCKER_LOG_FILE" ) &
    BACKEND_PID=$!
  fi

  if [ "$BUILD_FRONTEND" = true ]; then
    # HOST_URL is patched into the bundled api.js by site/scripts/docker-entrypoint.sh
    # at container start, not at build time.
    ( docker build \
        -t groupscape-web-frontend:latest \
        -t "groupscape-web-frontend:$VERSION" \
        -f site/Dockerfile \
        site 2>&1 | tag_lines frontend | timestamp >> "$DOCKER_LOG_FILE" ) &
    FRONTEND_PID=$!
  fi

  local BUILD_FAILED=false
  if [ -n "$BACKEND_PID" ]; then
    if wait "$BACKEND_PID"; then
      log_success "Backend image built (groupscape-web-backend:latest + $VERSION)"
    else
      log_error "Backend docker build failed"
      BUILD_FAILED=true
    fi
  fi
  if [ -n "$FRONTEND_PID" ]; then
    if wait "$FRONTEND_PID"; then
      log_success "Frontend image built (groupscape-web-frontend:latest + $VERSION)"
    else
      log_error "Frontend docker build failed"
      BUILD_FAILED=true
    fi
  fi
  if [ "$BUILD_FAILED" = true ]; then
    return 1
  fi

  # If a build was skipped, re-tag $VERSION onto the existing :latest so the
  # tag still exists for docker-compose to pick up and for the prune step below.
  if [ "$BUILD_BACKEND" = false ]; then
    docker tag groupscape-web-backend:latest "groupscape-web-backend:$VERSION"
  fi
  if [ "$BUILD_FRONTEND" = false ]; then
    docker tag groupscape-web-frontend:latest "groupscape-web-frontend:$VERSION"
  fi

  # Clean old versioned images (keep latest 2 per image; never touch "latest" or the "previous" rollback tag)
  for IMG in "${APP_IMAGES[@]}"; do
    OLD_IMAGES=$(docker image ls --format '{{.Tag}}' "$IMG" | grep -vE '^(latest|previous)$' | sort -V | head -n -2 || true)
    if [ -n "$OLD_IMAGES" ]; then
      log_info "Pruning old $IMG image tags: $(echo "$OLD_IMAGES" | tr '\n' ' ')"
      echo "$OLD_IMAGES" | xargs -I{} docker image rm "$IMG":{} 2>&1 | timestamp >> "$DOCKER_LOG_FILE" || true
    fi
  done

  # Ensure the database is up WITHOUT tearing anything down — the currently
  # running app keeps serving while we bring this up. In steady state this is a no-op.
  log_info "Ensuring database container is running..."
  if ! docker compose "${COMPOSE_ARGS[@]}" up -d groupscape-web-db 2>&1 | timestamp >> "$DOCKER_LOG_FILE"; then
    log_error "Failed to start groupscape-web-db — aborting (previous deployment left running)"
    restore_latest_tags
    return 1
  fi

  log_info "Waiting for database to be ready..."
  local DB_READY=false
  local DB_WAIT=0
  while [ $DB_WAIT -lt 60 ]; do
    if docker exec groupscape-web-db pg_isready -U "${PG_USER:-postgres}" -q 2>/dev/null; then
      DB_READY=true
      break
    fi
    sleep 2; DB_WAIT=$(( DB_WAIT + 2 ))
  done
  if [ "$DB_READY" != true ]; then
    log_error "Database did not become ready in time — aborting (previous deployment left running)"
    restore_latest_tags
    return 1
  fi

  # No separate migration step — groupscape-web-backend runs db::update_schema at
  # startup (server/src/main.rs), so a failed migration surfaces as a failed
  # health check below, not here.

  # The demo-reset sidecar (groupscape-web-demo-reset) only (re)seeds "@EXAMPLE" on its own
  # 6h loop, so a cold DB or a sidecar that silently died leaves /demo 401ing indefinitely
  # with nothing surfacing the failure. Run the seed synchronously once per deploy, using
  # the same image/env as the sidecar, so every successful deploy guarantees the demo group
  # exists — non-fatal to the overall deploy since the demo group isn't core functionality.
  log_info "Seeding demo group (@EXAMPLE)..."
  if docker compose "${COMPOSE_ARGS[@]}" run --rm --no-deps groupscape-web-demo-reset /app/seed 2>&1 | timestamp | tee -a "$DOCKER_LOG_FILE"; then
    log_success "Demo group seed completed"
  else
    log_error "Demo group seed failed — /demo may 401 until this is fixed (deploy continues)"
  fi

  # Recreate only the application containers with the new images. The DB keeps
  # running — no full teardown. The detached restarter is the safety net if this
  # process is killed mid-recreate.
  local RESUME_SCRIPT="/tmp/groupscape_web_resume_$$.sh"
  cat > "$RESUME_SCRIPT" <<RESUME
#!/usr/bin/env bash
sleep 30
# Safety net only. If the parent deploy process ($$) is still alive it will finish
# the recreate itself (and kill this script), so do nothing — this avoids a second
# 'up' racing the main one when the main 'up' takes longer than the sleep. Act only
# if the parent died mid-recreate (e.g. it redeployed its own image and restarted).
if kill -0 $$ 2>/dev/null; then
  rm -f "$RESUME_SCRIPT"
  exit 0
fi
cd "$REPO_DIR"
docker compose -f "$COMPOSE_FILE" up -d >> "$DOCKER_LOG_FILE" 2>&1
echo "[\$(date '+%Y-%m-%d %H:%M:%S')] [INFO] Detached restart completed (parent died mid-recreate)" >> "$DEPLOY_LOG_FILE"
rm -f "$RESUME_SCRIPT"
RESUME
  chmod +x "$RESUME_SCRIPT"
  setsid bash "$RESUME_SCRIPT" &>/dev/null &
  local RESUME_PID=$!
  disown $RESUME_PID
  log_info "Detached restart process forked (PID $RESUME_PID)"

  log_info "Recreating application containers..."
  if ! docker compose "${COMPOSE_ARGS[@]}" up -d --remove-orphans 2>&1 | timestamp >> "$DOCKER_LOG_FILE"; then
    log_error "docker compose up failed for $VERSION — initiating rollback"
    kill "$RESUME_PID" 2>/dev/null || pkill -f "$RESUME_SCRIPT" 2>/dev/null || true
    rm -f "$RESUME_SCRIPT"
    rollback || log_error "Automatic rollback did not fully succeed"
    return 1
  fi

  # Recreate succeeded — cancel the detached restarter.
  kill "$RESUME_PID" 2>/dev/null || pkill -f "$RESUME_SCRIPT" 2>/dev/null || true
  rm -f "$RESUME_SCRIPT"

  if ! check_health; then
    log_error "Health check failed for $VERSION — initiating rollback"
    rollback || log_error "Automatic rollback did not fully succeed"
    return 1
  fi

  # Reclaim disk from dangling layers left behind by repeated rebuilds.
  log_info "Pruning dangling image layers..."
  docker image prune -f 2>&1 | timestamp >> "$DOCKER_LOG_FILE" || true

  local DEPLOY_DURATION=$(( SECONDS - DEPLOY_START ))
  log_success "Deployment completed — $VERSION in ${DEPLOY_DURATION}s"
}

# Prints the topmost "## <date>" block of CHANGELOG.md, wrapped in markers Vantage
# scrapes out of the deploy log to build the #patch-notes embed (see
# server/lib/poller.js::notifyDeployWebhooks in the vantage repo). Silent no-op if
# the file is missing or has no dated block yet — deploy still succeeds either way.
emit_changelog() {
  local FILE="$REPO_DIR/CHANGELOG.md"
  [ -f "$FILE" ] || return 0
  local BLOCK
  BLOCK=$(awk '/^## /{n++} n==1' "$FILE")
  [ -n "$BLOCK" ] || return 0
  echo "[VANTAGE_CHANGELOG_START]"
  echo "$BLOCK"
  echo "[VANTAGE_CHANGELOG_END]"
}

record_deploy_success() {
  git rev-parse HEAD > "$STATE_FILE"
  FAILED_HASH=""
  FAILED_COUNT=0
  emit_changelog
}

record_deploy_failure() {
  # HEAD is the hash we actually attempted (the tree was synced before the failure).
  local H
  H=$(git rev-parse HEAD)
  if [ "$H" = "$FAILED_HASH" ]; then
    FAILED_COUNT=$(( FAILED_COUNT + 1 ))
  else
    FAILED_HASH="$H"
    FAILED_COUNT=1
  fi
  log_warn "Deploy of $H has now failed ${FAILED_COUNT}/${MAX_DEPLOY_RETRIES} time(s)"
}

run_cycle() {
  local RESULT=0
  if acquire_lock; then
    if [ "$FORCE_DEPLOY" = true ]; then
      log_warn "Force deploy enabled — skipping git checks"
      if deploy; then log_success "Forced deploy completed"; record_deploy_success
      else log_error "Forced deploy failed"; RESULT=1; record_deploy_failure; fi
    else
      if check_updates; then
        if deploy; then log_success "Deploy cycle completed"; record_deploy_success
        else log_error "Deploy cycle failed"; RESULT=1; record_deploy_failure; fi
      fi
    fi
    release_lock
  else
    log_warn "Skipped cycle — could not acquire lock"
    RESULT=1
  fi
  return $RESULT
}

if [ "$RUN_ONCE" = true ]; then
  log_info "Running in single-execution mode"
  # `if` context so a non-zero run_cycle is a clean exit, not a set -e abort.
  if run_cycle; then exit 0; else exit 1; fi
fi

while true; do
  # `|| true` so a failed cycle can never trip set -e and kill the watcher daemon.
  run_cycle || true
  sleep "$CHECK_INTERVAL"
done
