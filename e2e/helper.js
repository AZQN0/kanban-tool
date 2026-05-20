// Helper utilities for Kanban WebUI E2E tests.
// Starts a temporary kanban project, runs the test callback, then cleans up.

const { execFileSync, spawn } = require("child_process");
const fs = require("fs");
const http = require("http");
const net = require("net");
const path = require("path");
const os = require("os");

const BIN = path.resolve(__dirname, "../target/release/kanban");

let serverProcess = null;

async function freePort() {
  return await new Promise((resolve, reject) => {
    const server = net.createServer();
    server.listen(0, "127.0.0.1", () => {
      const address = server.address();
      server.close(() => {
        if (address && typeof address === "object") {
          resolve(address.port);
        } else {
          reject(new Error("Could not allocate free port"));
        }
      });
    });
    server.on("error", reject);
  });
}

function runKanban(args, opts = {}) {
  const kanbanBin = opts.bin || BIN;
  return execFileSync(kanbanBin, args, {
    cwd: opts.cwd || path.resolve(__dirname, ".."),
    timeout: opts.timeout || 10000,
    encoding: "utf8",
    stdio: opts.stdio || ["ignore", "pipe", "pipe"],
  });
}

async function waitForServer(port) {
  const deadline = Date.now() + 8000;
  let lastError;

  while (Date.now() < deadline) {
    try {
      const status = await new Promise((resolve, reject) => {
        const req = http.get(
          {
            hostname: "127.0.0.1",
            port,
            path: "/api/cards",
            timeout: 1000,
          },
          (res) => {
            res.resume();
            resolve(res.statusCode);
          }
        );
        req.on("timeout", () => {
          req.destroy(new Error("Timed out waiting for server"));
        });
        req.on("error", reject);
      });

      if (status === 200) {
        return;
      }
      lastError = new Error(`Unexpected status ${status}`);
    } catch (error) {
      lastError = error;
    }

    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  throw lastError || new Error("Server did not become ready");
}

/**
 * Start the webui server serving from a specific project directory.
 */
function startServer(projectDir, port, opts = {}) {
  return new Promise((resolve, reject) => {
    const kanbanBin = opts.bin || BIN;
    serverProcess = spawn(kanbanBin, ["web-ui", "--bind", "127.0.0.1", "--port", String(port)], {
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
  const tmpDir = fs.mkdtempSync(path.join(os.tmpdir(), "kanban-"));
  const kanbanBin = opts.bin || BIN;
  const port = opts.port || await freePort();

  let server;
  try {
    // Initialize kanban board
    runKanban(["init", tmpDir], { bin: kanbanBin });

    // Create test cards
    runKanban([
      "create",
      "--project",
      tmpDir,
      "--title",
      "Test Card 1",
      "--description",
      "First card",
      "--priority",
      "high",
      "--column",
      "backlog",
    ], { bin: kanbanBin });
    runKanban([
      "create",
      "--project",
      tmpDir,
      "--title",
      "Test Card 2",
      "--description",
      "Second card",
      "--priority",
      "medium",
      "--column",
      "backlog",
    ], { bin: kanbanBin });
    runKanban([
      "create",
      "--project",
      tmpDir,
      "--title",
      "Test Card 3",
      "--description",
      "Third card",
      "--priority",
      "low",
      "--column",
      "backlog",
    ], { bin: kanbanBin });

    if (opts.beforeStart) {
      await opts.beforeStart(tmpDir);
    }

    // Start server serving from the project directory
    server = await startServer(tmpDir, port, { bin: kanbanBin });
    await waitForServer(port);

    await opts.fn(tmpDir, port);
  } finally {
    stopServer();
    try { fs.rmSync(tmpDir, { recursive: true, force: true }); } catch {}
  }
}

module.exports = { withKanbanProject, startServer, stopServer, runKanban, freePort, BIN };
