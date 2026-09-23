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
      startServerReadyPattern: "Ready",
      puppeteerScript: "apps/workbench/scripts/lhci-puppeteer-auth.cjs",
      url: ["http://localhost:3000/workbench/queue"],
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
        chromeFlags: chromeArgs.join(" "),
      },
    },
    assert: {
      assertions: {
        "categories:performance": ["error", { minScore: 0.9 }],
      },
    },
  },
};
