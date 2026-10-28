import { useAppStore } from "@/features/app-shell";
import { useSettingsStore } from "@/features/settings";
import { ShortcutRuntime } from "@/features/shortcuts";
import { hasTauriInternals } from "@/shared/platform/tauri";
import { installOverflowFade } from "@/shared/ui";
import { platform as osPlatform } from "@tauri-apps/plugin-os";
import { useEffect, useLayoutEffect } from "react";
import { DesktopLayout } from "./layouts/DesktopLayout";
import { RenderErrorBoundary } from "./layouts/RenderErrorBoundary";
import { useDeepLinks } from "./useDeepLinks";

/** Kura is one window: the desktop frame around the file manager. */
export function App() {
  useLayoutEffect(() => installOverflowFade(), []);
  useEffect(() => {
    void useAppStore.getState().loadApp();
    void useSettingsStore.getState().load();
    if (!hasTauriInternals()) return;
    try {
      document.documentElement.dataset.osPlatform = osPlatform();
    } catch {
      // Platform detection is best-effort; leave the attribute unset.
    }
  }, []);
  useDeepLinks();

  return (
    <div className="app-pages-root h-full min-h-0 bg-charcoal-bg text-cream">
      <ShortcutRuntime />
      <RenderErrorBoundary>
        <DesktopLayout />
      </RenderErrorBoundary>
    </div>
  );
}
