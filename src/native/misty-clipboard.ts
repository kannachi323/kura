import { invoke } from "./invoke";

/** A clip from the Misty app's shared clipboard, as Misty lists it. */
export interface MistyClip {
  clipId: string;
  deviceName: string;
  fromThisDevice: boolean;
  kind: "empty" | "text" | "html" | "image" | "file_refs";
  preview: string;
  fileNames: string[];
  size: number;
  createdAt: number;
}

export interface MistyClipboard {
  /** "unavailable": Misty isn't running. "denied": not allowed in Misty. */
  state: "unavailable" | "denied" | "ready";
  view: {
    status: "off" | "locked" | "connecting" | "ready" | "unavailable";
    clips: MistyClip[];
  } | null;
}

export function mistyClipboardView(): Promise<MistyClipboard> {
  return invoke("misty_clipboard_view");
}

export function mistyClipboardCopy(clipId: string): Promise<void> {
  return invoke("misty_clipboard_copy", { clipId });
}

export function mistyClipboardPaste(
  clipId: string,
  directory: string,
): Promise<string[]> {
  return invoke("misty_clipboard_paste", { clipId, directory });
}
