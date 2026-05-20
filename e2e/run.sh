#!/usr/bin/env bash
# Run Playwright E2E tests for the Kanban WebUI.
# Each test creates its own temp project and starts its own server.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
PROJECT_DIR="$(dirname "$SCRIPT_DIR")"
BIN="$PROJECT_DIR/target/release/kanban"

# Check binary exists
if [ ! -x "$BIN" ]; then
  echo "❌ Binary not found at $BIN. Build first with: cargo build --release --features webui"
  exit 1
fi

# Kill any leftover server processes
fuser -k 9877/tcp 2>/dev/null || true
sleep 0.5

# Run Playwright tests (each test starts its own server via helper)
echo "▶️  Running Playwright E2E tests..."
npx playwright test "$@"
TEST_EXIT=$?

# Cleanup
fuser -k 9877/tcp 2>/dev/null || true

echo "🏁 E2E tests finished with exit code ${TEST_EXIT}"
exit $TEST_EXIT
