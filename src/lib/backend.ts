// Typed wrappers around the Tauri commands and events exposed by src-tauri.

import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type {
  PrinterAction,
  PrinterConfig,
  PrinterInput,
  PrinterUpdate,
  Snapshot,
} from "./types";

export const backend = {
  listPrinters: () => invoke<PrinterConfig[]>("list_printers"),
  getSnapshots: () => invoke<Record<string, Snapshot>>("get_snapshots"),
  addPrinter: (input: PrinterInput) => invoke<PrinterConfig>("add_printer", { input }),
  updatePrinter: (id: string, input: PrinterInput) =>
    invoke<PrinterConfig>("update_printer", { id, input }),
  removePrinter: (id: string) => invoke<void>("remove_printer", { id }),
  getActivePrinter: () => invoke<string | null>("get_active_printer"),
  setActivePrinter: (id: string | null) => invoke<void>("set_active_printer", { id }),
  action: (id: string, action: PrinterAction, arg?: string) =>
    invoke<void>("printer_action", { id, action, arg: arg ?? null }),
  testConnection: (input: PrinterInput) => invoke<string>("test_connection", { input }),
  hidePopover: () => invoke<void>("hide_popover"),
  quit: () => invoke<void>("quit"),

  onPrinterUpdate: (cb: (u: PrinterUpdate) => void): Promise<UnlistenFn> =>
    listen<PrinterUpdate>("printer:update", (e) => cb(e.payload)),
  onPrintersChanged: (cb: (list: PrinterConfig[]) => void): Promise<UnlistenFn> =>
    listen<PrinterConfig[]>("printers:changed", (e) => cb(e.payload)),
};
