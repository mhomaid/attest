const chromeArgs = [
  "--no-sandbox",
  "--disable-dev-shm-usage",
  "--headless=new",
  "--disable-gpu",
];

module.exports = {
  ci: {
    collect: {
      startServerCommand: "bash apps/workbench/scripts/lhci-start-server.sh",
      startServerReadyPattern: "LHCI_SERVER_READY",
      puppeteerScript: "apps/workbench/scripts/lhci-puppeteer-auth.cjs",
      url: ["http://127.0.0.1:3000/workbench/queue"],
      numberOfRuns: 1,
      puppeteerLaunchOptions: {
        args: chromeArgs,
        ...(process.env.CHROME_PATH ? { executablePath: process.env.CHROME_PATH } : {}),
      },
      settings: {
        preset: "desktop",
        formFactor: "desktop",
        screenEmulation: { disabled: true },
        ...(process.env.CHROME_PATH ? { chromePath: process.env.CHROME_PATH } : {}),
      },
    },
    assert: {
      assertions: {
        "categories:performance": ["error", { minScore: 0.9 }],
      },
    },
  },
};
