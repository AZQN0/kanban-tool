// @ts-check
const { test, expect } = require("@playwright/test");
const fs = require("fs");
const net = require("net");
const path = require("path");
const { withKanbanProject } = require("./helper");

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
      const response = await request.get(`http://127.0.0.1:${port}/api/cards`, {
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

test.describe("Kanban WebUI API errors", () => {
  test("returns explicit HTTP statuses for missing and invalid cards", async ({ request }) => {
    const port = await freePort();

    await withKanbanProject({
      port,
      fn: async (_tmpDir, serverPort) => {
        const baseUrl = `http://127.0.0.1:${serverPort}`;
        await waitForServer(request, serverPort);

        const missingGet = await request.get(`${baseUrl}/api/cards/not-a-card`);
        expect(missingGet.status()).toBe(404);
        expect((await missingGet.json()).error).toEqual(expect.any(String));

        const missingDelete = await request.delete(`${baseUrl}/api/cards/not-a-card`);
        expect(missingDelete.status()).toBe(404);
        expect((await missingDelete.json()).error).toEqual(expect.any(String));

        const unknownColumnCreate = await request.post(`${baseUrl}/api/cards`, {
          data: {
            title: "Bad column",
            column: "missing-column",
          },
        });
        expect(unknownColumnCreate.status()).toBe(400);
        expect((await unknownColumnCreate.json()).error).toEqual(expect.any(String));

        const missingMove = await request.post(`${baseUrl}/api/cards/not-a-card/move`, {
          data: {
            column: "todo",
          },
        });
        expect(missingMove.status()).toBe(404);
        expect((await missingMove.json()).error).toEqual(expect.any(String));

        const created = await request.post(`${baseUrl}/api/cards`, {
          data: {
            title: "Delete me",
            column: "todo",
          },
        });
        expect(created.status()).toBe(200);
        const createdBody = await created.json();

        const deleted = await request.delete(`${baseUrl}/api/cards/${createdBody.id}`);
        expect(deleted.status()).toBe(200);
        expect(await deleted.json()).toEqual({ success: true });

        const deletedAgain = await request.delete(`${baseUrl}/api/cards/${createdBody.id}`);
        expect(deletedAgain.status()).toBe(404);
      },
    });
  });

  test("returns internal error when markdown deletion fails", async ({ request }) => {
    const port = await freePort();

    await withKanbanProject({
      port,
      fn: async (tmpDir, serverPort) => {
        const baseUrl = `http://127.0.0.1:${serverPort}`;
        await waitForServer(request, serverPort);

        const created = await request.post(`${baseUrl}/api/cards`, {
          data: {
            title: "Delete with bad markdown path",
            column: "todo",
          },
        });
        expect(created.status()).toBe(200);
        const createdBody = await created.json();

        const cardPath = path.join(tmpDir, ".kanban", "cards", `${createdBody.id}.md`);
        fs.unlinkSync(cardPath);
        fs.mkdirSync(cardPath);

        const deleted = await request.delete(`${baseUrl}/api/cards/${createdBody.id}`);

        expect(deleted.status()).toBe(500);
        expect((await deleted.json()).error).toEqual(expect.any(String));
      },
    });
  });
});
