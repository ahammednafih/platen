// Typed wrappers over the Tauri commands in src-tauri/src/commands.rs.
import { invoke } from "@tauri-apps/api/core";

export type Fill = "Off" | "White" | "Paper";
export type Kind = "A4" | "Long";
export type ColorMode = "Color" | "Gray" | "BlackWhite";

export type SizeId = "A4" | "Letter" | "B5" | "Executive" | "A5" | "HalfLetter" | "Photo148x200" | "Photo5x8" | "A6" | "Photo5x7" | "Photo4x6" | "Photo3_5x5" | "Card" | "Glass";
export interface Word { text: string; x: number; y: number; w: number; h: number }
export interface PageText { lines: Word[][]; text: string }
export interface DetailRow { label: string; value: string; sub: boolean }
export interface DetailGroup { title: string; rows: DetailRow[] }
export interface Size { id: SizeId; name: string; w_mm: number; h_mm: number }
export interface Settings { dpi: number; mode: ColorMode; fill: Fill; size: SizeId }
export interface Preset { name: string; settings: Settings; builtin: boolean }
export interface Caps { resolutions: number[]; modes: ColorMode[]; sizes: Size[] }
export interface Page { file: string; kind: Kind; dpi: number; w: number; h: number; secs: number; color?: string | null }
export interface SessionView { name: string; dir: string; pages: Page[]; can_undo?: boolean; can_redo?: boolean }
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
  max_mb: number | null; searchable: boolean;
}
export type Edit = "RotateLeft" | "RotateRight" | "Rotate180" | "Straighten" | "Crop" | "Gray" | "BlackWhite" | "Colour"
  | { CropTo: { x: number; y: number; w: number; h: number } }
  | { Despeckle: { dpi: number } }
  | { Levels: { brightness: number; contrast: number } };
export interface Saved { session: string; name: string; path: string; bytes: number; modified: number }
export interface ShrinkResult { out: string; before: number; after: number; fits: boolean; pages: number }
export interface Exported { files: string[]; closed: boolean }

export type Quality = "Draft" | "Normal" | "High";
export type Scale = "Fit" | "Actual" | { Percent: number };
export type Subset = "All" | "Odd" | "Even";
export type Area = { Size: SizeId } | "HalfTop";
export interface PrintOpts { copies: number; color: boolean; media: string; scaling: string; quality: Quality }
export interface PrintJob {
  files: string[]; ranges: string; subset: Subset; reverse: boolean;
  nup: number; scale: Scale; collate: boolean; clean?: boolean; opts: PrintOpts;
}
export interface PrintCaps { media: string[]; media_default: string; color: boolean; qualities: Quality[]; max_copies: number; duplex: boolean }
export type SnmpMode = "ReadWrite" | "ReadOnly" | "Off";
export interface ButtonStatus { snmp: SnmpMode; destination: string | null; name: string; on: boolean }
export interface Press { func: string; seq: number }
export interface Plan { pages: number; picked: number; sheets: number; order: (number | null)[] }
export interface Progress { done: number; total: number }

type Slot = { replace?: number; at?: number };

/// Raw command results are an ArrayBuffer over the fast IPC channel, but a plain number array when
/// the webview falls back to message passing (as packaged builds can).
function bytes(data: ArrayBuffer | number[]): BlobPart {
  return data instanceof ArrayBuffer ? data : new Uint8Array(data);
}

export const api = {
  connect: (host?: string) => invoke<Device>("connect", { host }),
  status: () => invoke<DeviceStatus>("status"),
  capabilities: () => invoke<Caps>("capabilities"),
  drafts: () => invoke<Draft[]>("drafts"),
  openSession: (name?: string) => invoke<SessionView>("open_session", { name }),
  renameSession: (name: string) => invoke<SessionView>("rename_session", { name }),
  closeSession: () => invoke<void>("close_session"),
  getSettings: () => invoke<Settings>("get_settings"),
  setSettings: (settings: Settings) => invoke<void>("set_settings", { settings }),
  presets: () => invoke<Preset[]>("presets"),
  savePreset: (name: string) => invoke<Preset[]>("save_preset", { name }),
  deletePreset: (name: string) => invoke<Preset[]>("delete_preset", { name }),
  scanA4: (slot: Slot = {}) => invoke<PageResult>("scan_a4", slot),
  scanTop: () => invoke<TopHalf>("scan_top"),
  scanBottom: (top: TopHalf, slot: Slot = {}) => invoke<PageResult>("scan_bottom", { top, ...slot }),
  fillPage: (index: number) => invoke<PageResult>("fill_page", { index }),
  scanToPrint: () => invoke<string>("scan_to_print"),
  paperBounds: (index: number) => invoke<[number, number, number, number] | null>("paper_bounds", { index }),
  editPage: (index: number, edit: Edit) => invoke<PageResult>("edit_page", { index, edit }),
  ocrLanguages: () => invoke<string[]>("ocr_languages"),
  deletePage: (index: number) => invoke<SessionView>("delete_page", { index }),
  undo: () => invoke<SessionView>("undo"),
  redo: () => invoke<SessionView>("redo"),
  movePage: (from: number, to: number) => invoke<SessionView>("move_page", { from, to }),
  getExportOpts: () => invoke<ExportOpts>("get_export_opts"),
  export: (opts: ExportOpts) => invoke<Exported>("export", { opts }),
  library: () => invoke<Saved[]>("library"),
  deleteDraft: (name: string) => invoke<Draft[]>("delete_draft", { name }),
  filePages: (path: string) => invoke<number>("file_pages", { path }),
  shrinkPdf: (path: string, maxMb: number) => invoke<ShrinkResult>("shrink_pdf", { path, maxMb }),
  shareFile: (path: string, anchor: { x: number; y: number; w: number; h: number }) => invoke<void>("share_file", { path, anchor }),
  setDockIcon: (accent: string) => invoke<void>("set_dock_icon", { accent }),
  trashFile: (path: string) => invoke<void>("trash_file", { path }),
  renameFile: (path: string, name: string) => invoke<string>("rename_file", { path, name }),
  keepShrunk: (original: string, draft: string) => invoke<string>("keep_shrunk", { original, draft }),
  discardShrunk: (draft: string) => invoke<void>("discard_shrunk", { draft }),
  stageFile: async (f: File) =>
    invoke<string>("stage_file", new Uint8Array(await f.arrayBuffer()), { headers: { name: encodeURIComponent(f.name) } }),
  printCaps: () => invoke<PrintCaps>("print_caps"),
  printPlan: (job: PrintJob) => invoke<Plan>("print_plan", { job }),
  printPreview: async (job: PrintJob, slot: number) =>
    URL.createObjectURL(new Blob([bytes(await invoke<ArrayBuffer | number[]>("print_preview", { job, slot }))], { type: "image/jpeg" })),
  print: (job: PrintJob) => invoke<number>("print", { job }),
  copyReset: () => invoke<void>("copy_reset"),
  copyScan: (area: Area, color: boolean) => invoke<string>("copy_scan", { area, color }),
  copyLongTop: (color: boolean) => invoke<TopHalf>("copy_long_top", { color }),
  copyLongBottom: (top: TopHalf, color: boolean) => invoke<string>("copy_long_bottom", { top, color }),
  copyIdCard: (front: string, back: string) => invoke<string>("copy_id_card", { front, back }),
  deviceDetails: (password?: string) => invoke<DetailGroup[]>("device_details", { password }),
  enableSnmpWrite: (password?: string) => invoke<void>("enable_snmp_write", { password }),
  buttonStatus: (password?: string) => invoke<ButtonStatus>("button_status", { password }),
  buttonEnable: (password?: string) => invoke<void>("button_enable", { password }),
  buttonDisable: () => invoke<void>("button_disable"),
  scanButton: (at?: number) => invoke<PageResult>("scan_button", { at }),
  pageText: (file: string) => invoke<PageText>("page_text", { file }),
  stopJob: () => invoke<number>("stop_job"),
  copyTone: (files: string[], density: number, background: number) => invoke<string[]>("copy_tone", { files, density, background }),
};

/** True while a dialog or lightbox covers the screen, so screen shortcuts stay quiet. */
export const covered = () => !!document.querySelector(".backdrop");

/** Sizes offered before the scanner reports which fit its glass. */
export const DEFAULT_SIZES: Size[] = [
  { id: "A4", name: "A4", w_mm: 210, h_mm: 297 },
  { id: "Letter", name: "Letter", w_mm: 215.9, h_mm: 279.4 },
  { id: "A5", name: "A5", w_mm: 148, h_mm: 210 },
  { id: "A6", name: "A6", w_mm: 105, h_mm: 148 },
];

/** Readable name for an IPP media keyword, e.g. `iso_a4_210x297mm` → `A4`. */
export function paperName(kw: string): string {
  const named: Record<string, string> = { iso_a4: "A4", iso_a5: "A5", iso_a6: "A6", na_letter: "Letter", na_legal: "Legal", jis_b5: "B5", na_index: "Index card", na_5x7: "5×7 in", om_photo: "Photo" };
  const key = kw.split("_").slice(0, 2).join("_");
  const dims = kw.split("_").pop() ?? kw;
  return named[key] ? `${named[key]}` : dims.replace("x", " × ").replace(/(mm|in)$/, " $1");
}

/** Thumbnail URL for a session image. Edits write new files, so URLs never go stale. */
export function img(sub: "pages" | "raw", file: string, w = 360): string {
  return `scanimg://localhost/${sub}/${encodeURIComponent(file)}?w=${w}`;
}

/** Preview JPEG of one page of a saved PDF or image. */
export function fileImg(path: string, page = 0, w = 1600): string {
  return `scanfile://localhost/?path=${encodeURIComponent(path)}&page=${page}&w=${w}`;
}

export function size(bytes: number): string {
  return bytes >= 1e6 ? `${(bytes / 1e6).toFixed(1)} MB` : `${Math.max(1, Math.round(bytes / 1e3))} KB`;
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
