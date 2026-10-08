import { Button, Input, OptionSelect, Switch } from "@/shared/ui";
import { X } from "lucide-react";
import { useEffect, useState, type ReactNode } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  selectAppearancePreferences,
  selectFilePreferences,
  selectGeneralPreferences,
  selectSearchMaintenancePreferences,
} from "./preferences";
import { useSettingsStore } from "./useSettingsStore";

/** Kura's settings: one scrolling page with small section headers. */
export function SettingsPage({ onClose }: { onClose: () => void }) {
  const { document, error, update, removeOpenWithAssociation } =
    useSettingsStore(
      useShallow((state) => ({
        document: state.settings?.document ?? {},
        error: state.error,
        update: state.update,
        removeOpenWithAssociation: state.removeOpenWithAssociation,
      })),
    );
  const general = selectGeneralPreferences(document);
  const files = selectFilePreferences(document);
  const appearance = selectAppearancePreferences(document);
  const search = selectSearchMaintenancePreferences(document);
  const openWith = Object.entries(
    (document.open_with as Record<string, string> | undefined) ?? {},
  ).sort(([left], [right]) => left.localeCompare(right));

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  return (
    <div className="flex h-full min-h-0 flex-col bg-charcoal-bg text-cream">
      <header className="flex h-12 shrink-0 items-center justify-between border-b border-charcoal-border px-4">
        <h1 className="text-sm font-medium text-cream-bright">Settings</h1>
        <Button
          variant="ghost"
          size="sm"
          onClick={onClose}
          aria-label="Close settings"
        >
          <X size={16} />
        </Button>
      </header>
      <div className="min-h-0 flex-1 overflow-y-auto">
        <div className="mx-auto flex max-w-2xl flex-col gap-8 px-6 py-6">
          {error ? (
            <p role="alert" className="text-sm text-cream-muted">
              Settings could not be saved. {error}
            </p>
          ) : null}
          <Section title="Browsing">
            <Row
              title="Start folder"
              description="Where new tabs open. Leave empty for your home folder."
            >
              <PathInput
                value={general.preferredWorkspaceRoot}
                placeholder="Home folder"
                onCommit={(value) =>
                  void update("general", "preferred_workspace_root", value)
                }
              />
            </Row>
            <Row title="Default view">
              <OptionSelect
                className="w-36"
                aria-label="Default view"
                value={String(files.defaultViewModeIndex)}
                options={[
                  { value: "0", label: "List" },
                  { value: "1", label: "Grid" },
                ]}
                onValueChange={(value) =>
                  void update("files", "default_view_mode_index", Number(value))
                }
              />
            </Row>
            <Row title="Show hidden files">
              <Switch
                aria-label="Show hidden files"
                checked={files.showHiddenFiles}
                onCheckedChange={(checked) =>
                  void update("files", "show_hidden_files", checked)
                }
              />
            </Row>
            <Row
              title="Confirm before deleting"
              description="Ask before moving items to the Trash or deleting them."
            >
              <Switch
                aria-label="Confirm before deleting"
                checked={general.confirmDestructiveActions}
                onCheckedChange={(checked) =>
                  void update("general", "confirm_destructive_actions", checked)
                }
              />
            </Row>
          </Section>
          <Section title="Appearance">
            <Row title="Compact rows">
              <Switch
                aria-label="Compact rows"
                checked={appearance.compactModeEnabled}
                onCheckedChange={(checked) =>
                  void update("appearance", "compact_mode_enabled", checked)
                }
              />
            </Row>
            <Row
              title="Thumbnails"
              description="Show image previews in lists and grids."
            >
              <Switch
                aria-label="Thumbnails"
                checked={appearance.thumbnailPreviewsEnabled}
                onCheckedChange={(checked) =>
                  void update(
                    "appearance",
                    "thumbnail_previews_enabled",
                    checked,
                  )
                }
              />
            </Row>
          </Section>
          <Section title="Search">
            <Row
              title="Index files automatically"
              description="Kura keeps a private index on this device. Nothing is uploaded."
            >
              <Switch
                aria-label="Index files automatically"
                checked={search.automaticFileDiscoveryEnabled}
                onCheckedChange={(checked) =>
                  void update(
                    "search",
                    "automatic_file_discovery_enabled",
                    checked,
                  )
                }
              />
            </Row>
            <Row title="Index every">
              <OptionSelect
                className="w-36"
                aria-label="Index interval"
                value={String(search.discoveryIntervalMinutes)}
                options={[5, 15, 30, 60, 240].map((minutes) => ({
                  value: String(minutes),
                  label:
                    minutes < 60
                      ? `${minutes} minutes`
                      : `${minutes / 60} hours`,
                }))}
                onValueChange={(value) =>
                  void update(
                    "search",
                    "discovery_interval_minutes",
                    Number(value),
                  )
                }
              />
            </Row>
            <Row title="Include hidden files">
              <Switch
                aria-label="Include hidden files in search"
                checked={search.includeHidden}
                onCheckedChange={(checked) =>
                  void update("search", "include_hidden", checked)
                }
              />
            </Row>
            <Row title="Skip folders" description="One path per line.">
              <PathInput
                multiline
                value={search.ignoredPaths.join("\n")}
                placeholder="~/Library"
                onCommit={(value) =>
                  void update("search", "ignored_paths", value)
                }
              />
            </Row>
          </Section>
          <Section title="Open with">
            {openWith.length === 0 ? (
              <p className="text-sm text-cream-muted">
                Choose Open With on a file to remember an app for its type.
              </p>
            ) : (
              openWith.map(([extension, application]) => (
                <Row
                  key={extension}
                  title={extension}
                  description={application}
                >
                  <Button
                    variant="ghost"
                    size="sm"
                    onClick={() => void removeOpenWithAssociation(extension)}
                  >
                    Forget
                  </Button>
                </Row>
              ))
            )}
          </Section>
        </div>
      </div>
    </div>
  );
}

function Section(props: { title: string; children: ReactNode }) {
  return (
    <section className="flex flex-col gap-1">
      <h2 className="pb-1 text-xs font-medium text-cream-muted">
        {props.title}
      </h2>
      <div className="flex flex-col divide-y divide-charcoal-border">
        {props.children}
      </div>
    </section>
  );
}

function Row(props: {
  title: string;
  description?: string;
  children: ReactNode;
}) {
  return (
    <div className="flex min-h-12 items-center justify-between gap-6 py-2">
      <div className="min-w-0">
        <div className="text-sm text-cream-bright">{props.title}</div>
        {props.description ? (
          <div className="truncate text-xs text-cream-muted">
            {props.description}
          </div>
        ) : null}
      </div>
      <div className="shrink-0">{props.children}</div>
    </div>
  );
}

function PathInput(props: {
  value: string;
  placeholder: string;
  multiline?: boolean;
  onCommit: (value: string) => void;
}) {
  const [draft, setDraft] = useState(props.value);
  useEffect(() => setDraft(props.value), [props.value]);
  const commit = () => {
    if (draft !== props.value) props.onCommit(draft.trim());
  };
  if (props.multiline)
    return (
      <textarea
        className="h-20 w-64 resize-none rounded-md border border-charcoal-border bg-charcoal-card px-2 py-1 text-sm text-cream outline-none focus:border-charcoal-active"
        value={draft}
        placeholder={props.placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
      />
    );
  return (
    <Input
      className="w-64"
      value={draft}
      placeholder={props.placeholder}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={commit}
      onKeyDown={(event) => {
        if (event.key === "Enter") commit();
      }}
    />
  );
}
