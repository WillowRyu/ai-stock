import { describe, it, expect } from "vitest";
import { useToastStore } from "./toastStore";

describe("toastStore", () => {
  it("push adds, dismiss removes", () => {
    const id = useToastStore.getState().push({ kind: "error", title: "boom", ttl_ms: 0 });
    expect(useToastStore.getState().toasts.find((t) => t.id === id)).toBeTruthy();
    useToastStore.getState().dismiss(id);
    expect(useToastStore.getState().toasts.find((t) => t.id === id)).toBeUndefined();
  });
});
