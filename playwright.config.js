// @ts-check
const { defineConfig, devices } = require("@playwright/test");

/** @type {typeof import('@playwright/test')} */
module.exports = defineConfig({
  testDir: "./e2e",
  testMatch: /.*\.spec\.js/,
  timeout: 30_000,
  expect: { timeout: 5_000 },
  fullyParallel: false,
  forbidOnly: !!process.env.CI,
  retries: process.env.CI ? 1 : 0,
  workers: 1,
  reporter: "html",
  use: {
    baseURL: "http://127.0.0.1:9876",
    trace: "on-first-retry",
    screenshot: "only-on-failure",
    video: "retain-on-failure",
  },
  projects: [
    {
      name: "chromium",
      use: { ...devices["Desktop Chromium"] },
    },
  ],
});
