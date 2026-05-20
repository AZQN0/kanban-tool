// @ts-check
const { test, expect } = require("@playwright/test");
const { spawn } = require("child_process");
const net = require("net");
const { withKanbanProject, BIN } = require("./helper");

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

async function waitForServer(request, port) {
  const deadline = Date.now() + 8000;
  let lastError;

  while (Date.now() < deadline) {
    try {
      const response = await request.get(`http://127.0.0.1:${port}/static/app.js`, {
        timeout: 1000,
      });
      if (response.ok()) {
        return;
      }
      lastError = new Error(`Unexpected status ${response.status()}`);
    } catch (error) {
      lastError = error;
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }

  throw lastError || new Error("Server did not become ready");
}

test.describe("Kanban WebUI security", () => {
  test("serves static assets and blocks decoded traversal", async ({ request }) => {
    const port = await freePort();

    await withKanbanProject({
      port,
      fn: async (_tmpDir, serverPort) => {
        await waitForServer(request, serverPort);

        const appJs = await request.get(`http://127.0.0.1:${serverPort}/static/app.js`);
        expect(appJs.status()).toBe(200);
        expect(await appJs.text()).toContain("Application State");

        const traversal = await request.get(
          `http://127.0.0.1:${serverPort}/static/%2e%2e/%2e%2e/Cargo.toml`
        );
        expect([403, 404]).toContain(traversal.status());
        expect(await traversal.text()).not.toContain("[package]");
      },
    });
  });

  test("rejects remote bind unless explicitly allowed", async () => {
    const port = await freePort();

    const result = await new Promise((resolve, reject) => {
      const child = spawn(BIN, ["web-ui", "--bind", "0.0.0.0", "--port", String(port)], {
        cwd: __dirname,
        stdio: ["ignore", "pipe", "pipe"],
      });

      let stdout = "";
      let stderr = "";
      const timeout = setTimeout(() => {
        child.kill("SIGTERM");
        reject(new Error("web-ui kept running for remote bind without --allow-remote"));
      }, 3000);

      child.stdout.on("data", (data) => {
        stdout += data.toString();
      });
      child.stderr.on("data", (data) => {
        stderr += data.toString();
      });
      child.on("error", reject);
      child.on("close", (code) => {
        clearTimeout(timeout);
        resolve({ code, output: `${stdout}\n${stderr}` });
      });
    });

    expect(result.code).not.toBe(0);
    expect(result.output).toContain("Refusing to bind WebUI to non-loopback address");
    expect(result.output).toContain("--allow-remote");
  });
});
