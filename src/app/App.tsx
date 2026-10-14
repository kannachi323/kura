import { useAppStore } from "@/features/app-shell";
import { FilesPage } from "@/features/files/workspace";
import { kuraOpenSettingsEvent } from "@/features/files/workspace/explorer/workspace/ExplorerCommands";
import { SettingsPage, useSettingsStore } from "@/features/settings";
import { ShortcutRuntime } from "@/features/shortcuts";
import { installOverflowFade } from "@/shared/ui";
import { useEffect, useLayoutEffect, useState } from "react";
import { useDeepLinks } from "./useDeepLinks";

/** Kura is one window: the file manager, with settings over it when open. */
export function App() {
  const [settingsOpen, setSettingsOpen] = useState(false);
  useLayoutEffect(() => installOverflowFade(), []);
  useEffect(() => {
    void useAppStore.getState().loadApp();
    void useSettingsStore.getState().load();
    const open = () => setSettingsOpen(true);
    window.addEventListener(kuraOpenSettingsEvent, open);
    return () => window.removeEventListener(kuraOpenSettingsEvent, open);
  }, []);
  useDeepLinks();

  return (
    <div className="kura-app h-full min-h-0 bg-charcoal-bg">
      <ShortcutRuntime />
      <div className={settingsOpen ? "hidden" : "h-full min-h-0"}>
        <FilesPage active={!settingsOpen} />
      </div>
      {settingsOpen ? (
        <SettingsPage onClose={() => setSettingsOpen(false)} />
      ) : null}
    </div>
  );
}
