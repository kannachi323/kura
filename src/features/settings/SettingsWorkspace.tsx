import { Button, Input, Pressable, WorkspaceOverlay } from "@/shared/ui";
import { memo, useEffect, useMemo, useRef, useState } from "react";
import { DesktopSettingsFrame } from "./components/DesktopSettingsUI";
import { takeRequestedSettingsSection } from "./openSettings";
import { settingsPages, type SettingsSectionId } from "./sections";
import { useSettingsStore } from "./useSettingsStore";

interface SearchResult {
  key: string;
  label: string;
  page: SettingsSectionId;
  /** The row to focus once the page opens; empty for whole-page matches. */
  focus: string;
}

function searchSettings(query: string): SearchResult[] {
  const needle = query.trim().toLowerCase();
  if (!needle) return [];
  const pages = settingsPages
    .filter((page) => page.label.toLowerCase().includes(needle))
    .map((page) => ({
      key: `page:${page.id}`,
      label: page.label,
      page: page.id,
      focus: "",
    }));
  const rows = settingsPages.flatMap((page) =>
    page.rows
      .filter((row) => row.toLowerCase().includes(needle))
      .map((row) => ({
        key: `${page.id}:${row}`,
        label: row,
        page: page.id,
        focus: row,
      })),
  );
  return [...pages, ...rows];
}

/** Scrolls to, focuses, and briefly highlights the row a search result named. */
function useFocusSettingRow(focus: { label: string; request: number }) {
  const content = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!focus.label) return;
    let clearFlash: ReturnType<typeof setTimeout> | undefined;
    const timer = setTimeout(() => {
      const element = Array.from(
        content.current?.querySelectorAll<HTMLElement>(
          "[data-setting-label]",
        ) ?? [],
      ).find((row) => row.dataset.settingLabel === focus.label);
      if (!element) {
        content.current?.parentElement?.scrollTo?.({ top: 0 });
        return;
      }
      element.scrollIntoView?.({ block: "center" });
      const control =
        element.querySelector<HTMLElement>(
          "[data-setting-control] input:not(:disabled),[data-setting-control] button:not(:disabled),[data-setting-control] textarea",
        ) ?? element;
      control.focus({ preventScroll: true });
      element.dataset.settingFlash = "true";
      clearFlash = setTimeout(() => delete element.dataset.settingFlash, 900);
    }, 0);
    return () => {
      clearTimeout(timer);
      clearTimeout(clearFlash);
    };
  }, [focus]);
  return content;
}

export const SettingsWorkspace = memo(function SettingsWorkspace(props: {
  onClose?: () => void;
}) {
  const error = useSettingsStore((state) => state.error);
  const loaded = useSettingsStore((state) => state.loaded);
  const [active, setActive] = useState<SettingsSectionId>(() => {
    const requested = takeRequestedSettingsSection();
    return (
      settingsPages.find((candidate) => candidate.id === requested)?.id ??
      "general"
    );
  });
  const [query, setQuery] = useState("");
  const [focus, setFocus] = useState({ label: "", request: 0 });
  const content = useFocusSettingRow(focus);
  const results = useMemo(() => searchSettings(query), [query]);
  useEffect(() => {
    if (!loaded) void useSettingsStore.getState().load();
  }, [loaded]);
  const select = (id: SettingsSectionId, label = "") => {
    setActive(id);
    setFocus((current) => ({ label, request: current.request + 1 }));
    setQuery("");
  };
  const page =
    settingsPages.find((candidate) => candidate.id === active) ??
    settingsPages[0];
  const Page = page.Component;
  return (
    <DesktopSettingsFrame
      activeId={page.id}
      ariaLabel="Settings"
      items={settingsPages}
      navigationLabel="Settings sections"
      onClose={props.onClose}
      onSelect={(id) => select(id)}
      title={page.label}
      navigationHeader={
        <Input
          aria-label="Search settings"
          placeholder="Search"
          value={query}
          onChange={(event) => setQuery(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Escape" && query) {
              event.stopPropagation();
              setQuery("");
            }
            if (event.key === "Enter" && results[0])
              select(results[0].page, results[0].focus);
          }}
        />
      }
      navigationOverride={
        query.trim() ? (
          <div
            aria-label="Settings search results"
            className="kura-scrollbar grid min-h-0 flex-1 content-start gap-1 overflow-y-auto px-3 max-[680px]:px-2"
          >
            {results.map((result) => (
              <Pressable
                key={result.key}
                className="grid gap-0.5 rounded-md px-2.5 py-1.5 text-left hover:bg-charcoal-hover focus-visible:bg-charcoal-hover"
                onClick={() => select(result.page, result.focus)}
              >
                <span className="truncate text-[13px] text-cream">
                  {result.label}
                </span>
                {result.focus ? (
                  <span className="truncate text-[11px] text-cream-muted">
                    {settingsPages.find((p) => p.id === result.page)?.label}
                  </span>
                ) : null}
              </Pressable>
            ))}
            {!results.length ? (
              <p className="px-2.5 py-1.5 text-xs text-cream-muted">
                No settings match “{query.trim()}”
              </p>
            ) : null}
          </div>
        ) : undefined
      }
      contentHeader={
        error ? (
          <div
            role="alert"
            className="mb-5 flex items-center justify-between gap-3 text-sm text-cream"
          >
            <span>{error}</span>
            <Button
              variant="ghost"
              onClick={() => void useSettingsStore.getState().load()}
            >
              Retry
            </Button>
          </div>
        ) : null
      }
    >
      <div
        ref={content}
        key={page.id}
        className="animate-in fade-in-0 duration-150"
      >
        <Page />
      </div>
    </DesktopSettingsFrame>
  );
});

export function SettingsOverlay(props: { open: boolean; onClose: () => void }) {
  return (
    <WorkspaceOverlay {...props} ariaLabel="Settings">
      <SettingsWorkspace onClose={props.onClose} />
    </WorkspaceOverlay>
  );
}
