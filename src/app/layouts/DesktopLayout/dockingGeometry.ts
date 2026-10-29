import type { CSSProperties } from "react";

export const dockingMetrics = {
  tabStrip: 38,
  /** Gap between a horizontal tab strip's outer edge and its connected tabs. */
  tabInset: 3,
  sideTabs: 200,
  gap: 8,
  tab: 28,
};
export const dockingMotion = { duration: 300, easing: "ease-in-out" };
type ShellStyle = CSSProperties & Record<`--${string}`, string | number>;

/**
 * The only owner of shell tracks, native chrome insets and pane seams. Kura has
 * no rail: window tabs share the titlebar band beside the native window
 * controls, and panes fill the rest of the window.
 */
export function dockingGeometry({
  chromeLeft = 84,
  chromeRight = 0,
}: {
  /** Already adjusted for application zoom. */
  chromeLeft?: number;
  chromeRight?: number;
}) {
  const { gap, tabStrip } = dockingMetrics;
  const frame: ShellStyle = {
    gridTemplateColumns: "minmax(0, 1fr)",
    gridTemplateRows: "minmax(0, 1fr)",
    "--shell-titlebar": `${tabStrip}px`,
    "--shell-side-tabs": `${dockingMetrics.sideTabs}px`,
    "--shell-tab-strip-height": `${tabStrip}px`,
    "--shell-tab-inset": `${dockingMetrics.tabInset}px`,
    "--shell-tab-height": `${dockingMetrics.tab}px`,
    "--shell-gap": `${gap}px`,
    "--shell-motion-duration": `${dockingMotion.duration}ms`,
    "--shell-motion-easing": dockingMotion.easing,
  };
  // Only the tab strip borders the panes; their other edges meet the window.
  for (const edge of ["top", "right", "bottom", "left"] as const)
    frame[`--pane-seam-${edge}`] = edge === "top" ? "1px" : "0px";
  for (const y of ["top", "bottom"] as const)
    for (const x of ["left", "right"] as const)
      frame[`--pane-corner-${y}-${x}`] = "0px";
  return {
    frame,
    content: { gridColumn: 1, gridRow: 1 } satisfies CSSProperties,
    titlebarInsets: {
      animate: true,
      left: Math.max(gap, chromeLeft + gap),
      right: Math.max(0, chromeRight),
    },
  };
}
