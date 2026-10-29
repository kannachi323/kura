import { cn, IconButton } from "@/shared/ui";
import { X } from "lucide-react";
import type { ReactNode } from "react";
import {
  type DesktopSettingsNavEntry,
  SettingsNavigation,
} from "./SettingsNavigation";
export type { DesktopSettingsNavEntry } from "./SettingsNavigation";

/** Section sidebar beside a scrolling page of settings cards. */
export function DesktopSettingsFrame<Id extends string>(props: {
  activeId: Id;
  ariaLabel: string;
  children: ReactNode;
  items: readonly DesktopSettingsNavEntry<Id>[];
  navigationLabel: string;
  onClose?: () => void;
  onSelect: (id: Id) => void;
  title: ReactNode;
  navigationHeader?: ReactNode;
  /** Replaces the section list, e.g. with search results while the user types. */
  navigationOverride?: ReactNode;
  contentHeader?: ReactNode;
}) {
  return (
    <div
      aria-label={props.ariaLabel}
      className={cn(
        "grid h-full min-h-0 min-w-0 grid-cols-[216px_1px_minmax(0,1fr)] overflow-hidden bg-charcoal-bg",
        "max-[900px]:grid-cols-[184px_1px_minmax(0,1fr)]",
        "max-[680px]:grid-cols-[156px_1px_minmax(0,1fr)]",
      )}
    >
      <aside className="flex min-h-0 flex-col overflow-hidden bg-charcoal-sidebar py-3 text-cream-muted">
        {props.navigationHeader ? (
          <div className="mb-3 shrink-0 px-3 max-[680px]:px-2">
            {props.navigationHeader}
          </div>
        ) : null}
        {props.navigationOverride ?? (
          <SettingsNavigation
            items={props.items}
            activeId={props.activeId}
            label={props.navigationLabel}
            onSelect={props.onSelect}
          />
        )}
      </aside>

      <div aria-hidden="true" className="bg-charcoal-border" />

      <main className="flex min-h-0 min-w-0 flex-col bg-charcoal-bg">
        <header className="shrink-0 border-b border-charcoal-border">
          <div className="flex min-h-12 min-w-0 items-center gap-3 px-5 py-2 max-[720px]:px-4">
            <h1 className="min-w-0 flex-1 truncate text-base font-semibold leading-6 text-cream">
              {props.title}
            </h1>
            {props.onClose ? (
              <IconButton
                label={`Close ${props.ariaLabel.toLowerCase()}`}
                onClick={props.onClose}
              >
                <X className="size-4" strokeWidth={1.8} />
              </IconButton>
            ) : null}
          </div>
        </header>
        {/* The scroller spans the full pane so its scrollbar sits at the panel edge. */}
        <div className="kura-scrollbar min-h-0 min-w-0 flex-1 overflow-y-auto overscroll-contain">
          <div className="mx-auto w-full max-w-[760px] p-5 max-[720px]:p-4">
            {props.contentHeader}
            {props.children}
          </div>
        </div>
      </main>
    </div>
  );
}

export function DesktopSettingsSection(props: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <section
      aria-label={props.title}
      className="@container/settings mb-6 min-w-0 last:mb-0"
    >
      <div className="mb-2.5 min-w-0">
        <h2 className="text-[13px] font-medium leading-5 text-cream-muted">
          {props.title}
        </h2>
        {props.description ? (
          <p className="mt-1 max-w-2xl text-[13px] leading-[18px] text-cream-muted">
            {props.description}
          </p>
        ) : null}
      </div>
      <div className="overflow-hidden rounded-lg border border-charcoal-border bg-charcoal-card">
        {props.children}
      </div>
    </section>
  );
}

export function DesktopSettingsRow(props: {
  label: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div
      data-setting-label={props.label}
      tabIndex={-1}
      className={cn(
        "group/setting-row grid min-h-14 grid-cols-[minmax(0,0.52fr)_minmax(240px,0.48fr)] items-center gap-5",
        "border-b border-charcoal-border px-5 py-3 last:border-b-0 outline-none",
        "transition-colors duration-700 data-[setting-flash=true]:bg-charcoal-hover data-[setting-flash=true]:duration-0",
        "@max-[560px]/settings:grid-cols-1 @max-[560px]/settings:items-start @max-[560px]/settings:gap-3",
      )}
    >
      <div className="grid min-w-0 gap-0.5">
        <strong className="text-sm font-medium leading-5 text-cream">
          {props.label}
        </strong>
        {props.description ? (
          <span className="text-[13px] leading-[18px] text-cream-muted">
            {props.description}
          </span>
        ) : null}
      </div>
      <div
        data-setting-control
        className="flex min-w-0 items-center justify-end gap-2 @max-[560px]/settings:w-full @max-[560px]/settings:justify-start"
      >
        {props.children}
      </div>
    </div>
  );
}

export function SettingsNote(props: { children: ReactNode }) {
  return (
    <p className="m-0 px-5 py-4 text-[13px] leading-[18px] text-cream-muted">
      {props.children}
    </p>
  );
}
