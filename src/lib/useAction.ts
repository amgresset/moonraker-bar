import { useState } from "react";
import { backend } from "./backend";
import { useStore } from "@/store";
import type { PrinterAction } from "./types";

/** Runs a printer action and tracks which one is in flight (`"action"` or `"action:arg"`). */
export function useAction(printerId: string) {
  const [pending, setPending] = useState<string | null>(null);
  const run = async (action: PrinterAction, arg?: string) => {
    setPending(arg ? `${action}:${arg}` : action);
    try {
      await backend.action(printerId, action, arg);
    } catch (e) {
      useStore.getState().setError(String(e));
    } finally {
      setPending(null);
    }
  };
  return { pending, run };
}
