export function getTauriInternals(): TauriInternals | null {
  if (typeof window === "undefined") return null;
  return (
    (window as typeof window & { __TAURI_INTERNALS__?: TauriInternals })
      .__TAURI_INTERNALS__ ?? null
  );
}

export function hasTauriInternals(): boolean {
  return typeof getTauriInternals()?.invoke === "function";
}

export function safeTauriAssetUrl(path: string): string {
  const internals = getTauriInternals();
  if (internals?.convertFileSrc) {
    return internals.convertFileSrc(path);
  }
  return path;
}

export type TauriInternals = {
  convertFileSrc?: (filePath: string, protocol?: string) => string;
  invoke?: unknown;
};

/** True on macOS, where Finder's key conventions apply. */
export function isMacPlatform(): boolean {
  return (
    typeof navigator !== "undefined" &&
    /mac/i.test(navigator.platform || navigator.userAgent)
  );
}

/**
 * Extra windows (New Window) load with `?window=<label>&path=<folder>`.
 * Only the main window restores and saves the workspace layout; an extra
 * window starts at `path` and is forgotten when closed.
 */
export function secondaryWindow(): {
  label: string;
  path: string | null;
} | null {
  if (typeof window === "undefined") return null;
  const params = new URLSearchParams(window.location.search);
  const label = params.get("window");
  return label ? { label, path: params.get("path") } : null;
}
