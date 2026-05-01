module.exports = {
  ci: {
    collect: {
      startServerCommand: "bash apps/workbench/scripts/lhci-start-server.sh",
      startServerReadyPattern: "Ready",
      puppeteerScript: "apps/workbench/scripts/lhci-puppeteer-auth.cjs",
      url: ["http://localhost:3000/workbench/queue"],
      numberOfRuns: 1,
      settings: {
        preset: "desktop",
        formFactor: "desktop",
        screenEmulation: { disabled: true },
      },
    },
    assert: {
      assertions: {
        "categories:performance": ["error", { minScore: 0.9 }],
      },
    },
  },
};
