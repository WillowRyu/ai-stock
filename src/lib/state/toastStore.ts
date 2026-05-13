import { create } from "zustand";

export type ToastKind = "info" | "warning" | "error";

export interface Toast {
  id: number;
  kind: ToastKind;
  title: string;
  body?: string;
  /** Auto-dismiss timeout in milliseconds. Use 0 to disable auto-dismiss. */
  ttl_ms: number;
}

interface State {
  toasts: Toast[];
  push(t: Omit<Toast, "id">): number;
  dismiss(id: number): void;
}

let nextId = 1;

export const useToastStore = create<State>((set, get) => ({
  toasts: [],
  push(t) {
    const id = nextId++;
    const toast: Toast = { id, ...t };
    set((p) => ({ toasts: [...p.toasts, toast] }));
    if (t.ttl_ms > 0) {
      setTimeout(() => get().dismiss(id), t.ttl_ms);
    }
    return id;
  },
  dismiss(id) {
    set((p) => ({ toasts: p.toasts.filter((t) => t.id !== id) }));
  },
}));
