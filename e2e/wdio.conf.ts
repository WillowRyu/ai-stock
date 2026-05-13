// WebdriverIO config for tauri-driver E2E smoke.
// `as any` cast keeps this stub portable across @wdio/types versions; the real
// CI run will refine types if/when we standardise a wdio version.
export const config = {
  runner: "local",
  specs: ["./specs/**/*.e2e.ts"],
  maxInstances: 1,
  capabilities: [
    {
      "tauri:options": { application: "../target/debug/ai-stock-app" },
    },
  ],
  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: { ui: "bdd", timeout: 60000 },
  hostname: "127.0.0.1",
  port: 4444,
  services: [],
  beforeSession: () => {
    require("child_process").spawn("tauri-driver", [], { stdio: "inherit" });
  },
} as any;
