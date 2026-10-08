import type {
  AppSnapshot,
  ClipboardPayload,
  ClipboardSnapshot,
  PasteItem,
} from "@/native/ipc";

import { invoke } from "./invoke";
export function appSnapshot(): Promise<AppSnapshot> {
  return invoke("app_snapshot");
}

export function clipboardSnapshot(): Promise<ClipboardSnapshot> {
  return invoke("clipboard_snapshot");
}

export function clipboardSetLocal(
  payload: ClipboardPayload,
): Promise<ClipboardPayload> {
  return invoke("clipboard_set_local", { payload });
}

export function clipboardNativeFileRefs(): Promise<PasteItem[]> {
  return invoke("clipboard_native_file_refs");
}

export function clipboardWriteFileRefs(items: PasteItem[]): Promise<boolean> {
  return invoke("clipboard_write_file_refs", { items });
}

export function clipboardWriteFileBytes(
  items: Array<{ name: string; bytes: number[] }>,
): Promise<boolean> {
  return invoke("clipboard_write_file_bytes", { items });
}
