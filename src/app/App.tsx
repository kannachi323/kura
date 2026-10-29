import { useAppStore } from "@/features/app-shell";
import {
  selectAppearancePreferences,
  useSettingsStore,
} from "@/features/settings";
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
  const theme = useSettingsStore(
    (state) => selectAppearancePreferences(state.settings?.document).theme,
  );
  useLayoutEffect(() => {
    // "system" leaves the attribute off so the CSS media query decides.
    if (theme === "system") delete document.documentElement.dataset.theme;
    else document.documentElement.dataset.theme = theme;
  }, [theme]);

  return (
    <div className="app-pages-root h-full min-h-0 bg-charcoal-bg text-cream">
      <ShortcutRuntime />
      <RenderErrorBoundary>
        <DesktopLayout />
      </RenderErrorBoundary>
    </div>
  );
}
