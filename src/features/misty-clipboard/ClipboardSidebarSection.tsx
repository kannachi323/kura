import {
  mistyClipboardCopy,
  mistyClipboardPaste,
  mistyClipboardView,
  type MistyClip,
  type MistyClipboard,
} from "@/native/misty-clipboard";
import { SidebarSectionHeader, sidebarStyles } from "@/features/file-ui";
import { errorText } from "@/shared/lib/format";
import {
  Button,
  Collapsible,
  CollapsibleContent,
  SkeletonList,
} from "@/shared/ui";
import { ClipboardList, FileText, Image } from "lucide-react";
import { useCallback, useEffect, useState } from "react";

const openStorageKey = "kura.sidebar.clipboard.open";
const refreshMs = 5_000;

const viewText: Record<string, string> = {
  off: "Turn on Clipboard for this device in Misty’s Devices settings.",
  locked: "Unlock sync in Misty to share your clipboard.",
  connecting: "Connecting to your shared clipboard.",
  unavailable: "Your shared clipboard can’t be reached right now.",
};

function readOpen(): boolean {
  try {
    return window.localStorage.getItem(openStorageKey) !== "false";
  } catch {
    return true;
  }
}

/**
 * The clipboard shared across the person's Misty devices, read through the
 * Misty app on this computer. Kura itself has no account; without Misty this
 * section only explains how to turn it on.
 */
export function ClipboardSidebarSection(props: {
  activePath: string;
  onPasted: (directory: string) => void;
}) {
  const [open, setOpen] = useState(readOpen);
  const [clipboard, setClipboard] = useState<MistyClipboard | null>(null);
  const [message, setMessage] = useState("");
  const refresh = useCallback(() => {
    void mistyClipboardView()
      .then(setClipboard)
      .catch(() => setClipboard({ state: "unavailable", view: null }));
  }, []);

  useEffect(() => {
    if (!open) return;
    refresh();
    const timer = window.setInterval(refresh, refreshMs);
    return () => window.clearInterval(timer);
  }, [open, refresh]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    try {
      window.localStorage.setItem(openStorageKey, String(next));
    } catch {
      // Private storage modes keep the default.
    }
  };

  const canPasteHere =
    props.activePath.startsWith("/") ||
    /^[A-Za-z]:[\\/]/.test(props.activePath);
  const paste = (clip: MistyClip) => {
    setMessage("");
    void mistyClipboardPaste(clip.clipId, props.activePath)
      .then((paths) => {
        setMessage(
          paths.length === 1
            ? "Pasted into this folder."
            : `Pasted ${paths.length} items.`,
        );
        props.onPasted(props.activePath);
      })
      .catch((error: unknown) => setMessage(errorText(error)));
  };
  const copy = (clip: MistyClip) => {
    setMessage("");
    void mistyClipboardCopy(clip.clipId)
      .then(() => setMessage("Copied. Paste anywhere."))
      .catch((error: unknown) => setMessage(errorText(error)));
  };

  return (
    <Collapsible className={sidebarStyles.section} open={open}>
      <SidebarSectionHeader
        title="Clipboard"
        collapsed={!open}
        onToggle={toggle}
      />
      <CollapsibleContent className={sidebarStyles.list}>
        {clipboard === null ? (
          <SkeletonList
            label="Loading clipboard"
            rows={2}
            lines={2}
            className="px-2"
          />
        ) : clipboard.state === "unavailable" ? (
          <p className={sidebarStyles.muted}>
            Open Misty and sign in to share your clipboard across devices.
          </p>
        ) : clipboard.state === "denied" ? (
          <p className={sidebarStyles.muted}>
            Allow Kura when Misty asks to show your clipboard here.
          </p>
        ) : (
          <>
            {clipboard.view && viewText[clipboard.view.status] ? (
              <p className={sidebarStyles.muted}>
                {viewText[clipboard.view.status]}
              </p>
            ) : null}
            {clipboard.view?.status === "ready" &&
            clipboard.view.clips.length === 0 ? (
              <p className={sidebarStyles.muted}>
                Copy something on any of your devices.
              </p>
            ) : null}
            {clipboard.view?.clips.map((clip) => (
              <ClipRow
                key={clip.clipId}
                clip={clip}
                canPasteHere={canPasteHere}
                onPaste={() => paste(clip)}
                onCopy={() => copy(clip)}
              />
            ))}
          </>
        )}
        {message ? (
          <p role="status" className={sidebarStyles.muted}>
            {message}
          </p>
        ) : null}
      </CollapsibleContent>
    </Collapsible>
  );
}

function ClipRow(props: {
  clip: MistyClip;
  canPasteHere: boolean;
  onPaste: () => void;
  onCopy: () => void;
}) {
  const { clip } = props;
  const Icon =
    clip.kind === "file_refs"
      ? FileText
      : clip.kind === "image"
        ? Image
        : ClipboardList;
  const title =
    clip.kind === "file_refs"
      ? clip.fileNames.join(", ") || "Files"
      : clip.kind === "image"
        ? "Image"
        : clip.preview || "Text";
  return (
    <div className="group/clip grid gap-1 rounded-md px-2.5 py-1.5 hover:bg-charcoal-hover">
      <div className="flex min-w-0 items-center gap-2.5">
        <Icon
          size={16}
          aria-hidden="true"
          className="flex-none text-cream-muted"
        />
        <span className="min-w-0 flex-1 truncate text-[13px] font-medium text-cream-bright">
          {title}
        </span>
      </div>
      <div className="flex items-center gap-1 pl-6">
        <span className="min-w-0 flex-1 truncate text-[11px] text-cream-muted">
          {clip.fromThisDevice
            ? "This device"
            : clip.deviceName || "Another device"}
        </span>
        {props.canPasteHere ? (
          <Button variant="toolbar" size="xs" onClick={props.onPaste}>
            Paste here
          </Button>
        ) : null}
        <Button variant="toolbar" size="xs" onClick={props.onCopy}>
          Copy
        </Button>
      </div>
    </div>
  );
}
