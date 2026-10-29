import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type PointerEvent,
  type RefObject,
} from "react";

export interface MarqueeRect {
  left: number;
  top: number;
  right: number;
  bottom: number;
}

/** How close to the top or bottom edge a drag starts scrolling, in px. */
const EDGE = 28;
/** A press that moves less than this is a click, not a marquee. */
const THRESHOLD = 4;

/**
 * Rubber-band selection: dragging on empty space in a file list draws a
 * rectangle and selects what it touches. Shift or Cmd/Ctrl adds to the
 * current selection. Rectangles are in scroll-content coordinates, so items
 * the list has virtualised away are still hit, and dragging past the top or
 * bottom edge scrolls.
 */
export function useMarqueeSelection(options: {
  scrollRef: RefObject<HTMLDivElement | null>;
  enabled: boolean;
  hitTest: (rect: MarqueeRect) => string[];
  selectedIds: readonly string[];
  onSelectIds?: (ids: string[]) => void;
}) {
  const [box, setBox] = useState<MarqueeRect | null>(null);
  const latest = useRef(options);
  latest.current = options;
  const drag = useRef<{
    pointerId: number;
    startX: number;
    startY: number;
    clientX: number;
    clientY: number;
    base: string[];
    lastKey: string;
    active: boolean;
    frame: number | null;
  } | null>(null);

  const update = useCallback(() => {
    const state = drag.current;
    const element = latest.current.scrollRef.current;
    if (!state || !element) return;
    const bounds = element.getBoundingClientRect();
    const x = state.clientX - bounds.left + element.scrollLeft;
    const y = state.clientY - bounds.top + element.scrollTop;
    if (
      !state.active &&
      Math.hypot(x - state.startX, y - state.startY) < THRESHOLD
    )
      return;
    state.active = true;
    const rect = {
      left: Math.min(state.startX, x),
      top: Math.min(state.startY, y),
      right: Math.max(state.startX, x),
      bottom: Math.max(state.startY, y),
    };
    setBox(rect);
    const hits = latest.current.hitTest(rect);
    const ids = state.base.length
      ? [...new Set([...state.base, ...hits])]
      : hits;
    const key = ids.join("\n");
    if (key !== state.lastKey) {
      state.lastKey = key;
      latest.current.onSelectIds?.(ids);
    }
  }, []);

  const autoScroll = useCallback(() => {
    const state = drag.current;
    const element = latest.current.scrollRef.current;
    if (!state || !element) return;
    state.frame = null;
    const bounds = element.getBoundingClientRect();
    const step =
      state.clientY < bounds.top + EDGE
        ? -Math.ceil((bounds.top + EDGE - state.clientY) / 2)
        : state.clientY > bounds.bottom - EDGE
          ? Math.ceil((state.clientY - (bounds.bottom - EDGE)) / 2)
          : 0;
    if (step !== 0 && state.active) {
      element.scrollTop += step;
      update();
      state.frame = window.requestAnimationFrame(autoScroll);
    }
  }, [update]);

  const end = useCallback(() => {
    const state = drag.current;
    if (state?.frame != null) window.cancelAnimationFrame(state.frame);
    drag.current = null;
    setBox(null);
  }, []);
  useEffect(() => end, [end]);

  const onPointerDown = useCallback((event: PointerEvent<HTMLDivElement>) => {
    const { enabled, onSelectIds, scrollRef, selectedIds } = latest.current;
    const element = scrollRef.current;
    if (!enabled || !onSelectIds || !element || event.button !== 0) return;
    const target = event.target as HTMLElement;
    if (target.closest("[data-entry-id], input, textarea, button, th")) return;
    const bounds = element.getBoundingClientRect();
    // Presses on the scrollbars scroll; they don't select.
    if (
      event.clientX - bounds.left > element.clientWidth ||
      event.clientY - bounds.top > element.clientHeight
    )
      return;
    const additive = event.shiftKey || event.metaKey || event.ctrlKey;
    if (!additive) onSelectIds([]);
    element.focus({ preventScroll: true });
    element.setPointerCapture(event.pointerId);
    drag.current = {
      pointerId: event.pointerId,
      startX: event.clientX - bounds.left + element.scrollLeft,
      startY: event.clientY - bounds.top + element.scrollTop,
      clientX: event.clientX,
      clientY: event.clientY,
      base: additive ? [...selectedIds] : [],
      lastKey: additive ? selectedIds.join("\n") : "",
      active: false,
      frame: null,
    };
  }, []);

  const onPointerMove = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      const state = drag.current;
      if (!state || state.pointerId !== event.pointerId) return;
      state.clientX = event.clientX;
      state.clientY = event.clientY;
      update();
      if (state.frame == null)
        state.frame = window.requestAnimationFrame(autoScroll);
    },
    [autoScroll, update],
  );

  const onPointerUp = useCallback(
    (event: PointerEvent<HTMLDivElement>) => {
      if (drag.current?.pointerId !== event.pointerId) return;
      event.currentTarget.releasePointerCapture(event.pointerId);
      end();
    },
    [end],
  );

  return {
    box,
    handlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp,
      onPointerCancel: onPointerUp,
    },
  };
}
