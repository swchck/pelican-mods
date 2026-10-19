import { defineConfig, devices } from "@playwright/test";

// the app's window is WebKit (WKWebView), so the screens are checked in WebKit against
// the browser mock; Tauri itself isn't involved
export default defineConfig({
  testDir: "e2e",
  fullyParallel: true,
  reporter: "list",
  use: { baseURL: "http://localhost:1421", ...devices["Desktop Safari"], viewport: { width: 1100, height: 720 } },
  projects: [{ name: "webkit", use: { browserName: "webkit" } }],
  webServer: {
    command: "npx vite --port 1421 --strictPort",
    url: "http://localhost:1421",
    reuseExistingServer: true,
  },
});
