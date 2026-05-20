// Helper utilities for Kanban WebUI E2E tests.
// Starts a temporary kanban project, runs the test callback, then cleans up.

const { execSync, spawn } = require("child_process");
const fs = require("fs");
const path = require("path");
const os = require("os");

const BIN = path.resolve(__dirname, "../target/release/kanban");

let serverProcess = null;

/**
 * Start the webui server serving from a specific project directory.
 */
function startServer(projectDir, port) {
  return new Promise((resolve, reject) => {
    serverProcess = spawn(BIN, ["web-ui", "--bind", "127.0.0.1", "--port", String(port)], {
      cwd: projectDir,
      stdio: ["pipe", "pipe", "pipe"],
      env: { ...process.env },
    });

    serverProcess.stderr.on("data", () => {}); // discard stderr
    serverProcess.stdout.on("data", () => {}); // discard stdout

    serverProcess.on("error", reject);
    serverProcess.on("close", (code) => {
      if (code !== 0 && code !== null) {
        reject(new Error(`Server exited with code ${code}`));
      }
    });

    resolve(serverProcess);
  });
}

function stopServer() {
  if (serverProcess) {
    serverProcess.kill("SIGTERM");
    serverProcess = null;
  }
}

/**
 * Create a temporary kanban project, run the test callback, then clean up.
 */
async function withKanbanProject(opts) {
  const projectName = opts.projectName || `kanban-e2e-${Date.now()}`;
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "kanban-"));
  const kanbanBin = opts.bin || BIN;
  const port = opts.port || 9877; // Use different port to avoid conflicts

  let server;
  try {
    // Initialize kanban board
    execSync(`${kanbanBin} init "${tmpDir}"`, {
      cwd: path.resolve(__dirname, ".."),
      timeout: 10000,
    });

    // Create test cards
    execSync(
      `${kanbanBin} create --project "${tmpDir}" --title "Test Card 1" --description "First card" --priority high --column backlog`,
      { cwd: path.resolve(__dirname, ".."), timeout: 10000 }
    );
    execSync(
      `${kanbanBin} create --project "${tmpDir}" --title "Test Card 2" --description "Second card" --priority medium --column backlog`,
      { cwd: path.resolve(__dirname, ".."), timeout: 10000 }
    );
    execSync(
      `${kanbanBin} create --project "${tmpDir}" --title "Test Card 3" --description "Third card" --priority low --column backlog`,
      { cwd: path.resolve(__dirname, ".."), timeout: 10000 }
    );

    // Start server serving from the project directory
    server = await startServer(tmpDir, port);

    await opts.fn(tmpDir, port);
  } finally {
    stopServer();
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch {}
  }
}

module.exports = { withKanbanProject, startServer, stopServer, BIN };
