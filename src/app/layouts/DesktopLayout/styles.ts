// The tab inset and titlebar controls move on the same timeline.
export const shellMotionClass =
  "kura-shell-motion motion-reduce:transition-none";

/** Room for the macOS traffic lights at the start of the titlebar band. */
export const desktopTitlebarInset = 84;
export const windowsTitlebarInset = 8;

export function desktopTitlebarGeometry(
  appZoom: number,
  inset = desktopTitlebarInset,
): { left: number; scale: number } {
  const zoom = Number.isFinite(appZoom) && appZoom > 0 ? appZoom : 1;
  return { left: inset / zoom, scale: 1 / zoom };
}

export const windowsTitlebarControlsClass =
  "pointer-events-auto absolute right-0 top-0 z-[3] flex h-full w-max flex-nowrap items-center";

export const windowsTitlebarControlButtonClass =
  "grid h-full w-[46px] shrink-0 place-items-center border-0 bg-transparent p-0 text-cream-muted transition-colors hover:bg-charcoal-hover hover:text-cream";
export const windowsTitlebarCloseButtonClass = `${windowsTitlebarControlButtonClass} hover:bg-charcoal-active hover:text-cream-bright`;
