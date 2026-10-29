import { useEffect, useRef, type RefObject } from "react";

const revealEntryEvent = "kura:reveal-entry";

interface RevealEntryDetail {
  paneId: string;
  entryId: string;
}

/** Asks the pane's list or grid to scroll an entry into view and focus it. */
export function revealEntry(paneId: string, entryId: string): void {
  window.dispatchEvent(
    new CustomEvent<RevealEntryDetail>(revealEntryEvent, {
      detail: { paneId, entryId },
    }),
  );
}

/**
 * Scrolls a windowed list so a revealed entry is visible, then focuses its
 * row once the window has rendered it, so keyboard focus follows selection.
 */
export function useRevealEntry(options: {
  paneId: string;
  entryIds: () => string[];
  scrollRef: RefObject<HTMLDivElement | null>;
  bounds: (index: number) => { top: number; bottom: number };
}): void {
  // The entry may not be rendered yet (a store update in the same tick), so
  // the request waits for the next renders, briefly.
  const pending = useRef<{ entryId: string; expiresAtMs: number } | null>(null);
  const scrolled = useRef(false);
  const latest = useRef(options);
  latest.current = options;

  useEffect(() => {
    const handle = (event: Event) => {
      const { paneId, entryId } = (event as CustomEvent<RevealEntryDetail>)
        .detail;
      if (paneId !== latest.current.paneId) return;
      pending.current = { entryId, expiresAtMs: Date.now() + 2000 };
      scrolled.current = false;
      revealPending();
    };
    window.addEventListener(revealEntryEvent, handle);
    return () => window.removeEventListener(revealEntryEvent, handle);
  }, []);

  useEffect(() => {
    revealPending();
  });

  function revealPending() {
    const request = pending.current;
    const current = latest.current;
    const element = current.scrollRef.current;
    if (!request || !element) return;
    if (Date.now() > request.expiresAtMs) {
      pending.current = null;
      return;
    }
    if (!scrolled.current) {
      const index = current.entryIds().indexOf(request.entryId);
      if (index < 0) return;
      const { top, bottom } = current.bounds(index);
      if (top < element.scrollTop) element.scrollTo({ top });
      else if (bottom > element.scrollTop + element.clientHeight) {
        element.scrollTo({ top: bottom - element.clientHeight });
      }
      scrolled.current = true;
    }
    const row = element.querySelector<HTMLElement>(
      `[data-entry-id="${CSS.escape(request.entryId)}"]`,
    );
    if (!row) return;
    pending.current = null;
    row.focus({ preventScroll: true });
  }
}
