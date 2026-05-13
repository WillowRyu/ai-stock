import { browser, $ } from "@wdio/globals";

// `as any` casts keep this stub portable across @wdio/types versions; CI will
// refine if/when we standardise a wdio version.
describe("ai-stock golden path", () => {
  it("starts and shows watchlist heading", async () => {
    await (browser as any).pause(2000);
    const heading = await ($ as any)("text=ai-stock");
    await heading.waitForDisplayed({ timeout: 10000 });
  });
});
