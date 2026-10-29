import { openSettingsEvent, SettingsOverlay } from "@/features/settings";
import { useShortcutHandler } from "@/features/shortcuts";
import {
  appZoomRenderScale,
  useAppZoom,
  useAppZoomValue,
} from "@/shared/hooks/useAppZoom";
import { cn, Pressable } from "@/shared/ui";
import { Minus, Square, X } from "lucide-react";
import { useCallback, useLayoutEffect, useRef, useState } from "react";
import "./docking.css";
import { dockingGeometry } from "./dockingGeometry";
import { RestoreGlyph } from "./RestoreGlyph";
import * as styles from "./styles";
import { useDesktopWindowChrome } from "./useDesktopWindowChrome";
import { WorkspaceCanvas } from "./WorkspaceCanvas";

/**
 * Kura's desktop frame: window tabs share the titlebar band beside the traffic
 * lights, and split panes fill the rest. Files views carry their own sidebar,
 * which holds the workspace switcher and Settings.
 */
export function DesktopLayout() {
  const {
    shouldShowWindowsTitlebarControls: windowsControls,
    isWindowMaximized,
    handleDesktopTitlebarPointerDown,
    toggleTitlebarMaximize,
    minimizeTitlebarWindow,
    closeTitlebarWindow,
  } = useDesktopWindowChrome();
  const zoom = useAppZoom();
  useShortcutHandler("app.zoom_in", zoom.zoomIn);
  useShortcutHandler("app.zoom_out", zoom.zoomOut);
  useShortcutHandler("app.zoom_reset", zoom.resetZoom);
  const appZoom = appZoomRenderScale(useAppZoomValue());
  const [settingsOpen, setSettingsOpen] = useState(false);
  const openSettings = useCallback(() => setSettingsOpen(true), []);
  const closeSettings = useCallback(() => setSettingsOpen(false), []);
  useShortcutHandler("app.open_settings", openSettings);
  useLayoutEffect(() => {
    window.addEventListener(openSettingsEvent, openSettings);
    return () => window.removeEventListener(openSettingsEvent, openSettings);
  }, [openSettings]);

  const windowsControlsRef = useRef<HTMLDivElement>(null);
  const [windowsControlsWidth, setWindowsControlsWidth] = useState(0);
  useLayoutEffect(() => {
    const element = windowsControlsRef.current;
    const measure = () => setWindowsControlsWidth(element?.offsetWidth ?? 0);
    measure();
    const observer = new ResizeObserver(measure);
    if (element) observer.observe(element);
    return () => observer.disconnect();
  }, [windowsControls]);

  const titlebar = styles.desktopTitlebarGeometry(
    appZoom,
    windowsControls ? styles.windowsTitlebarInset : styles.desktopTitlebarInset,
  );
  const geometry = dockingGeometry({
    chromeLeft: titlebar.left,
    chromeRight: windowsControls ? (windowsControlsWidth || 140) / appZoom : 0,
  });

  return (
    <main
      className={cn(
        "kura-docking-frame",
        "transition-[grid-template-columns,grid-template-rows]",
        styles.shellMotionClass,
      )}
      style={geometry.frame}
      data-kura-desktop-frame
      data-tab-position="top"
      onPointerDown={(event) => {
        const target = event.target instanceof Element ? event.target : null;
        if (!target?.closest("[data-kura-window-titlebar-region='true']"))
          return;
        handleDesktopTitlebarPointerDown(event);
      }}
    >
      <header
        className="kura-docking-titlebar"
        data-shared-tabs="true"
        data-shared-chrome="true"
        onPointerDown={handleDesktopTitlebarPointerDown}
      >
        {/* The corner beside the traffic lights drags the window. */}
        <div
          className={cn(
            "kura-docking-titlebar-drag-region",
            "transition-[width]",
            styles.shellMotionClass,
          )}
          aria-hidden="true"
          style={{ width: geometry.titlebarInsets.left }}
        />
        {windowsControls ? (
          <div
            ref={windowsControlsRef}
            className={styles.windowsTitlebarControlsClass}
            data-kura-window-drag-block="true"
            style={{
              transform: `scale(${titlebar.scale})`,
              transformOrigin: "top right",
            }}
          >
            <Pressable
              aria-label="Minimize window"
              className={styles.windowsTitlebarControlButtonClass}
              title="Minimize"
              onClick={minimizeTitlebarWindow}
            >
              <Minus size={16} strokeWidth={1.5} />
            </Pressable>
            <Pressable
              className={styles.windowsTitlebarControlButtonClass}
              aria-label={
                isWindowMaximized ? "Restore window" : "Maximize window"
              }
              title={isWindowMaximized ? "Restore" : "Maximize"}
              onClick={() =>
                void toggleTitlebarMaximize().catch(() => undefined)
              }
            >
              {isWindowMaximized ? (
                <RestoreGlyph />
              ) : (
                <Square size={16} strokeWidth={1.5} />
              )}
            </Pressable>
            <Pressable
              aria-label="Close window"
              className={styles.windowsTitlebarCloseButtonClass}
              title="Close"
              onClick={closeTitlebarWindow}
            >
              <X size={16} strokeWidth={1.5} />
            </Pressable>
          </div>
        ) : null}
      </header>

      <section
        className="route-shell relative z-10 min-h-0 min-w-0 overflow-hidden bg-charcoal-bg"
        style={geometry.content}
      >
        <WorkspaceCanvas titlebarInsets={geometry.titlebarInsets} />
      </section>

      <SettingsOverlay open={settingsOpen} onClose={closeSettings} />
      <AppZoomIndicator
        visible={zoom.indicatorVisible}
        percent={zoom.zoomPercent}
      />
    </main>
  );
}

function AppZoomIndicator(props: { visible: boolean; percent: number }) {
  return (
    <div
      className={cn(
        "pointer-events-none fixed inset-x-0 top-10 layer-dialog-backdrop mx-auto w-fit rounded-full",
        "border border-charcoal-border bg-charcoal-card px-3 py-1.5 text-sm font-semibold text-cream",
        "shadow-xl transition duration-150",
        props.visible
          ? "translate-y-0 opacity-100"
          : "-translate-y-2 opacity-0",
      )}
      role="status"
      aria-live="polite"
      aria-hidden={!props.visible}
    >
      {props.percent}%
    </div>
  );
}
