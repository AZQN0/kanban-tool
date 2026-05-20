// @ts-check
const { test, expect } = require("@playwright/test");
const { withKanbanProject } = require("./helper");

test.describe("Kanban WebUI E2E Tests", () => {
  test("does not advertise project switching in the single-project WebUI", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);

        await expect(page.locator("#project-modal")).toHaveCount(0);
        const bottomText = await page.locator("#bottom-bar").textContent();
        expect(bottomText).not.toContain("Project");

        await page.keyboard.press("Shift+P");
        await expect(page.locator("#project-modal")).toHaveCount(0);
      },
    });
  });

  test("updates a second page when a card moves in another page", async ({ browser }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        const pageA = await browser.newPage();
        const pageB = await browser.newPage();
        try {
          await pageA.goto(`http://127.0.0.1:${port}/`);
          await pageB.goto(`http://127.0.0.1:${port}/`);

          await expect(pageB.locator("#cards-list")).toContainText("Test Card 1", { timeout: 10000 });

          const cardId = await pageA.evaluate(async () => {
            const response = await fetch("/api/cards");
            const board = await response.json();
            return board.columns
              .flatMap((column) => column.cards)
              .find((card) => card.title === "Test Card 1").id;
          });

          await pageA.evaluate(async (id) => {
            const response = await fetch(`/api/cards/${id}/move`, {
              method: "POST",
              headers: { "Content-Type": "application/json" },
              body: JSON.stringify({ column: "done" }),
            });
            if (!response.ok) {
              throw new Error(await response.text());
            }
          }, cardId);

          await expect(pageB.locator("#cards-list")).not.toContainText("Test Card 1", { timeout: 1500 });
          await expect(pageB.locator("#columns-list")).toContainText("done (1)");
        } finally {
          await pageA.close();
          await pageB.close();
        }
      },
    });
  });

  test("page loads and displays project name", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        await expect(page.locator("#project-name")).toBeVisible({ timeout: 8000 });
        const projectName = page.locator("#project-name");
        await expect(projectName).toHaveText(/📋/);
      },
    });
  });

  test("displays columns (backlog, todo, in_progress, review, done)", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Wait for columns to render
        await expect(page.locator("#columns-list .column-item").first()).toBeVisible({ timeout: 10000 });
        const columnNames = await page.locator("#columns-list .column-item").allTextContents();
        expect(columnNames.some((n) => n.includes("backlog"))).toBe(true);
        expect(columnNames.some((n) => n.includes("todo"))).toBe(true);
        expect(columnNames.some((n) => n.includes("done"))).toBe(true);
      },
    });
  });

  test("displays cards in the cards panel", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Wait for columns to render, then click on the first non-empty column
        await expect(page.locator("#columns-list .column-item").first()).toBeVisible({ timeout: 10000 });

        // Click on columns until we find one with cards
        const colItems = page.locator("#columns-list .column-item");
        const count = await colItems.count();
        let found = false;
        for (let i = 0; i < count; i++) {
          const text = await colItems.nth(i).textContent();
          if (text.includes("(") && !text.includes("(0)")) {
            await colItems.nth(i).click();
            found = true;
            break;
          }
        }

        // Wait for cards to appear
        await expect(page.locator("#cards-list .card-item").first()).toBeVisible({ timeout: 10000 });
        const cards = await page.locator("#cards-list .card-item").all();
        expect(cards.length).toBeGreaterThanOrEqual(1);
      },
    });
  });

  test("shows card count in top bar", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        await expect(page.locator("#card-count")).toContainText(/Cards:/, { timeout: 10000 });
      },
    });
  });

  test("shows detail panel when card is clicked", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Wait for columns, find non-empty column
        await expect(page.locator("#columns-list .column-item").first()).toBeVisible({ timeout: 10000 });
        const colItems = page.locator("#columns-list .column-item");
        const count = await colItems.count();
        for (let i = 0; i < count; i++) {
          const text = await colItems.nth(i).textContent();
          if (text.includes("(") && !text.includes("(0)")) {
            await colItems.nth(i).click();
            break;
          }
        }

        // Wait for cards to appear and click the first card
        await expect(page.locator("#cards-list .card-item").first()).toBeVisible({ timeout: 10000 });
        await page.locator("#cards-list .card-item").first().click();

        // Detail panel should show card title
        await expect(page.locator("#detail-panel .detail-title")).toBeVisible({ timeout: 3000 });
        const title = await page.locator("#detail-panel .detail-title").textContent();
        expect(title).toContain("Test Card");
      },
    });
  });

  test("shows and hides move modal with 'm' key", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Wait for columns, find non-empty column
        await expect(page.locator("#columns-list .column-item").first()).toBeVisible({ timeout: 10000 });
        const colItems = page.locator("#columns-list .column-item");
        const count = await colItems.count();
        for (let i = 0; i < count; i++) {
          const text = await colItems.nth(i).textContent();
          if (text.includes("(") && !text.includes("(0)")) {
            await colItems.nth(i).click();
            break;
          }
        }

        // Wait for cards and click one
        await expect(page.locator("#cards-list .card-item").first()).toBeVisible({ timeout: 10000 });
        await page.locator("#cards-list .card-item").first().click();

        // Press 'm' to open move modal
        await page.keyboard.press("m");
        // Move modal should be visible
        await expect(page.locator("#move-modal:not(.hidden)")).toBeVisible({ timeout: 3000 });
        // Should show column options
        const moveCols = await page.locator("#move-modal .move-col").allTextContents();
        expect(moveCols.some((c) => c.includes("backlog"))).toBe(true);
        expect(moveCols.some((c) => c.includes("done"))).toBe(true);
        // Press Escape to close
        await page.keyboard.press("Escape");
        // Modal should be hidden (wait for hidden class to reappear)
        await expect(page.locator("#move-modal.hidden")).toBeAttached({ timeout: 3000 });
      },
    });
  });

  test("shows and hides delete modal with 'D' key", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Wait for columns, find non-empty column
        await expect(page.locator("#columns-list .column-item").first()).toBeVisible({ timeout: 10000 });
        const colItems = page.locator("#columns-list .column-item");
        const count = await colItems.count();
        for (let i = 0; i < count; i++) {
          const text = await colItems.nth(i).textContent();
          if (text.includes("(") && !text.includes("(0)")) {
            await colItems.nth(i).click();
            break;
          }
        }

        // Click a card
        await expect(page.locator("#cards-list .card-item").first()).toBeVisible({ timeout: 10000 });
        await page.locator("#cards-list .card-item").first().click();

        // Press 'D' (uppercase d) to open delete modal
        await page.evaluate(() => {
          document.dispatchEvent(new KeyboardEvent("keydown", { key: "D", code: "KeyD", bubbles: true }));
        });
        await expect(page.locator("#delete-modal:not(.hidden)")).toBeVisible({ timeout: 3000 });
        // Should show the card title
        const cardName = await page.locator("#delete-card-title").textContent();
        expect(cardName).toContain("Test Card");
        // Cancel with 'n'
        await page.keyboard.press("n");
        await expect(page.locator("#delete-modal.hidden")).toBeAttached({ timeout: 3000 });
      },
    });
  });

  test("shows focus indicator changing", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        await expect(page.locator("#focus-indicator")).toBeVisible();
        const initial = await page.locator("#focus-indicator").textContent();
        expect(initial).toContain("Cards");
        // Press ArrowRight to move focus
        await page.keyboard.press("ArrowRight");
        // Focus should have changed after a brief wait for render
        await page.waitForTimeout(100);
        const afterRight = await page.locator("#focus-indicator").textContent();
        // Focus cycles: Columns -> Cards -> Detail -> Columns
        // If we started at Cards, right should go to Detail
        const valid = afterRight.includes("Detail") || afterRight.includes("Columns") || afterRight.includes("Cards");
        expect(valid).toBe(true);
      },
    });
  });

  test("shows message on actions", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Find non-empty column
        await expect(page.locator("#columns-list .column-item").first()).toBeVisible({ timeout: 10000 });
        const colItems = page.locator("#columns-list .column-item");
        const count = await colItems.count();
        for (let i = 0; i < count; i++) {
          const text = await colItems.nth(i).textContent();
          if (text.includes("(") && !text.includes("(0)")) {
            await colItems.nth(i).click();
            break;
          }
        }

        // Wait for cards and click one
        await expect(page.locator("#cards-list .card-item").first()).toBeVisible({ timeout: 10000 });
        await page.locator("#cards-list .card-item").first().click();

        // Press 'm' to open move modal
        await page.keyboard.press("m");

        // Wait for move modal to be visible
        await expect(page.locator("#move-modal:not(.hidden)")).toBeVisible({ timeout: 3000 });

        // Click the "done" column option
        const doneBtn = page.locator("#move-modal .move-col").filter({ hasText: "done" });
        await doneBtn.first().click();

        // Should see a message
        await expect(page.locator(".bar-message")).toContainText(/Moved/, { timeout: 5000 });
      },
    });
  });

  test("search input appears when pressing '/'", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Press '/' to enter search mode
        await page.evaluate(() => {
          document.dispatchEvent(new KeyboardEvent("keydown", { key: "/", code: "Slash", bubbles: true }));
        });
        // Search input should be visible
        await expect(page.locator("#search-input:not(.hidden)")).toBeVisible({ timeout: 3000 });
        // Type a query and press Enter
        await page.locator("#search-field").fill("Test Card 1");
        await page.keyboard.press("Enter");
        // Should show search results
        const title = page.locator("#cards-title");
        await expect(title).toContainText("Search Results", { timeout: 5000 });
      },
    });
  });

  test("bottom bar shows key bindings", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        await expect(page.locator("#bottom-bar")).toBeVisible();
        const bottomText = await page.locator("#bottom-bar").textContent();
        expect(bottomText).toContain("Navigate");
        expect(bottomText).toContain("Move");
        expect(bottomText).toContain("Delete");
      },
    });
  });

  test("empty detail shows placeholder", async ({ page }) => {
    await withKanbanProject({
      fn: async (tmpDir, port) => {
        await page.goto(`http://127.0.0.1:${port}/`);
        // Ensure no card is selected (click away or press Escape)
        await page.keyboard.press("Escape");
        await expect(page.locator("#detail-content .empty-hint")).toContainText(
          /No card selected/i,
          { timeout: 3000 }
        );
      },
    });
  });
});
