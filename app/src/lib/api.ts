// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";

export type Fill = "Off" | "White" | "Paper";
export type Kind = "A4" | "Long";
export type ColorMode = "Color" | "Gray" | "BlackWhite";

export interface Settings { dpi: number; mode: ColorMode; fill: Fill }
export interface Caps { resolutions: number[]; modes: ColorMode[] }
export interface Page { file: string; kind: Kind; dpi: number; w: number; h: number; secs: number }
export interface SessionView { name: string; dir: string; pages: Page[] }
export interface PageResult { session: SessionView; index: number; note: string }
export interface TopHalf { path: string; stem: string; secs: number }
export interface Device { name: string; host: string }
export interface Ink { name: string; level: number }
export interface PrinterStatus { model: string; state: string; reasons: string[]; ink: Ink[] }
export interface DeviceStatus { scanner: string; printer: PrinterStatus | null }
export interface Draft { name: string; pages: number; modified: number }
export interface ExportOpts {
  color: boolean; share: boolean; print: boolean; images: boolean;
  cleanup: boolean; open_pdf: boolean; open_folder: boolean;
}
export interface Exported { files: string[]; closed: boolean }

type Slot = { replace?: number; at?: number };

export const api = {
  connect: (host?: string) => invoke<Device>("connect", { host }),
  status: () => invoke<DeviceStatus>("status"),
  capabilities: () => invoke<Caps>("capabilities"),
  drafts: () => invoke<Draft[]>("drafts"),
  openSession: (name?: string) => invoke<SessionView>("open_session", { name }),
  closeSession: () => invoke<void>("close_session"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  scanA4: (slot: Slot = {}) => invoke<PageResult>("scan_a4", slot),
  scanTop: () => invoke<TopHalf>("scan_top"),
  scanBottom: (top: TopHalf, slot: Slot = {}) => invoke<PageResult>("scan_bottom", { top, ...slot }),
  fillPage: (index: number) => invoke<PageResult>("fill_page", { index }),
  deletePage: (index: number) => invoke<SessionView>("delete_page", { index }),
  movePage: (from: number, to: number) => invoke<SessionView>("move_page", { from, to }),
  getExportOpts: () => invoke<ExportOpts>("get_export_opts"),
  export: (opts: ExportOpts) => invoke<Exported>("export", { opts }),
};

/** Thumbnail URL for a session image. Edits write new files, so URLs never go stale. */
export function img(sub: "pages" | "raw", file: string, w = 360): string {
  return `scanimg://localhost/${sub}/${encodeURIComponent(file)}?w=${w}`;
}

export function mm(p: Page): [number, number] {
  const f = 25.4 / p.dpi;
  return [Math.round(p.w * f), Math.round(p.h * f)];
}

export function dur(secs: number): string {
  const s = Math.round(secs);
  if (s < 60) return `${s}s`;
  if (s < 3600) return `${Math.floor(s / 60)}m ${String(s % 60).padStart(2, "0")}s`;
  return `${Math.floor(s / 3600)}h ${String(Math.floor((s % 3600) / 60)).padStart(2, "0")}m`;
}
