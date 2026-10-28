import { useAppStore } from "@/features/app-shell";
import {
  detectShortcutPlatform,
  formatShortcutLabel,
  shortcutCommandRegistry,
  useEffectiveShortcut,
} from "@/features/shortcuts";
import { Button, Input, OptionSelect, Switch, Textarea } from "@/shared/ui";
import { CloudAccountsSettings } from "@/features/cloud/CloudAccountsSettings";
import {
  Cloud,
  FolderCog,
  Info,
  Keyboard,
  Palette,
  Search,
  SquareArrowOutUpRight,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useState, type ComponentType } from "react";
import { useShallow } from "zustand/react/shallow";
import {
  DesktopSettingsRow as Row,
  DesktopSettingsSection as Section,
  SettingsNote,
} from "./components/DesktopSettingsUI";
import {
  selectAppearancePreferences,
  selectFilePreferences,
  selectGeneralPreferences,
  selectSearchMaintenancePreferences,
  selectShortcutPreferences,
} from "./preferences";
import { useSettingsStore } from "./useSettingsStore";

/** Stable while settings load, so selectors do not see a new object each render. */
const emptyDocument: Record<string, unknown> = {};

export type SettingsSectionId =
  | "general"
  | "cloud"
  | "appearance"
  | "search"
  | "shortcuts"
  | "open-with"
  | "about";

export interface SettingsPage {
  id: SettingsSectionId;
  label: string;
  icon: LucideIcon;
  breakBefore?: boolean;
  /** Row labels, so search can jump straight to one. */
  rows: string[];
  Component: ComponentType;
}

function useSettingsDocument() {
  return useSettingsStore(
    useShallow((state) => ({
      document: state.settings?.document ?? emptyDocument,
      update: state.update,
    })),
  );
}

function GeneralSection() {
  const { document, update } = useSettingsDocument();
  const general = selectGeneralPreferences(document);
  const files = selectFilePreferences(document);
  return (
    <>
      <Section title="Browsing">
        <Row
          label="Start folder"
          description="Where new tabs open. Leave empty for your home folder."
        >
          <CommitInput
            value={general.preferredWorkspaceRoot}
            placeholder="Home folder"
            onCommit={(value) =>
              void update("general", "preferred_workspace_root", value)
            }
          />
        </Row>
        <Row label="Default view">
          <OptionSelect
            className="w-40"
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
        <Row label="Show hidden files">
          <Switch
            aria-label="Show hidden files"
            checked={files.showHiddenFiles}
            onCheckedChange={(checked) =>
              void update("files", "show_hidden_files", checked)
            }
          />
        </Row>
      </Section>
      <Section title="Safety">
        <Row
          label="Confirm before deleting"
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
    </>
  );
}

function AppearanceSection() {
  const { document, update } = useSettingsDocument();
  const appearance = selectAppearancePreferences(document);
  return (
    <Section title="Lists and grids">
      <Row label="Compact rows" description="Fit more items on screen.">
        <Switch
          aria-label="Compact rows"
          checked={appearance.compactModeEnabled}
          onCheckedChange={(checked) =>
            void update("appearance", "compact_mode_enabled", checked)
          }
        />
      </Row>
      <Row
        label="Thumbnails"
        description="Show image previews in lists and grids."
      >
        <Switch
          aria-label="Thumbnails"
          checked={appearance.thumbnailPreviewsEnabled}
          onCheckedChange={(checked) =>
            void update("appearance", "thumbnail_previews_enabled", checked)
          }
        />
      </Row>
    </Section>
  );
}

function SearchSection() {
  const { document, update } = useSettingsDocument();
  const search = selectSearchMaintenancePreferences(document);
  return (
    <Section
      title="Index"
      description="Kura keeps a private index on this device. Nothing is uploaded."
    >
      <Row label="Index files automatically">
        <Switch
          aria-label="Index files automatically"
          checked={search.automaticFileDiscoveryEnabled}
          onCheckedChange={(checked) =>
            void update("search", "automatic_file_discovery_enabled", checked)
          }
        />
      </Row>
      <Row label="Index every">
        <OptionSelect
          className="w-40"
          aria-label="Index interval"
          value={String(search.discoveryIntervalMinutes)}
          options={[5, 15, 30, 60, 240].map((minutes) => ({
            value: String(minutes),
            label:
              minutes < 60
                ? `${minutes} minutes`
                : `${minutes / 60} hour${minutes > 60 ? "s" : ""}`,
          }))}
          onValueChange={(value) =>
            void update("search", "discovery_interval_minutes", Number(value))
          }
        />
      </Row>
      <Row label="Include hidden files">
        <Switch
          aria-label="Include hidden files in search"
          checked={search.includeHidden}
          onCheckedChange={(checked) =>
            void update("search", "include_hidden", checked)
          }
        />
      </Row>
      <Row label="Skip folders" description="One path per line.">
        <CommitInput
          multiline
          value={search.ignoredPaths.join("\n")}
          placeholder="~/Library"
          onCommit={(value) => void update("search", "ignored_paths", value)}
        />
      </Row>
    </Section>
  );
}

function ShortcutsSection() {
  const { document, update } = useSettingsDocument();
  const hints = selectShortcutPreferences(document).shortcutHintsEnabled;
  const categories = [
    ...new Set(shortcutCommandRegistry.map((command) => command.category)),
  ];
  return (
    <>
      <Section title="Hints">
        <Row
          label="Show shortcut hints"
          description="Show key combinations in menus and tooltips."
        >
          <Switch
            aria-label="Show shortcut hints"
            checked={hints}
            onCheckedChange={(checked) =>
              void update("shortcuts", "shortcut_hints_enabled", checked)
            }
          />
        </Row>
      </Section>
      {categories.map((category) => (
        <Section key={category} title={category}>
          {shortcutCommandRegistry
            .filter((command) => command.category === category)
            .map((command) => (
              <Row
                key={command.id}
                label={command.label}
                description={command.description}
              >
                <ShortcutKeys commandId={command.id} />
              </Row>
            ))}
        </Section>
      ))}
    </>
  );
}

function ShortcutKeys({ commandId }: { commandId: string }) {
  const binding = useEffectiveShortcut(commandId);
  const label = formatShortcutLabel(binding.primary, detectShortcutPlatform());
  return label ? (
    <kbd className="rounded-md border border-charcoal-border bg-charcoal-bg px-2 py-0.5 font-sans text-xs text-cream">
      {label}
    </kbd>
  ) : (
    <span className="text-xs text-cream-muted">Not set</span>
  );
}

function OpenWithSection() {
  const { associations, remove } = useSettingsStore(
    useShallow((state) => ({
      associations: state.settings?.document?.open_with as
        Record<string, string> | undefined,
      remove: state.removeOpenWithAssociation,
    })),
  );
  const entries = Object.entries(associations ?? {}).sort(([a], [b]) =>
    a.localeCompare(b),
  );
  return (
    <Section title="Remembered apps">
      {entries.length === 0 ? (
        <SettingsNote>
          Choose Open With on a file to remember an app for its type.
        </SettingsNote>
      ) : (
        entries.map(([extension, application]) => (
          <Row key={extension} label={extension} description={application}>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void remove(extension)}
            >
              Forget
            </Button>
          </Row>
        ))
      )}
    </Section>
  );
}

function AboutSection() {
  const app = useAppStore((state) => state.app);
  return (
    <>
      <Section title="Kura">
        <Row label="Version">
          <span className="text-sm text-cream tabular-nums">
            {app?.version ?? "—"}
          </span>
        </Row>
        <Row
          label="Settings file"
          description="Kura stores its settings on this device."
        >
          <span
            className="max-w-[320px] truncate text-right text-[13px] text-cream-muted"
            title={app?.environment.settingsPath}
          >
            {app?.environment.settingsPath ?? "—"}
          </span>
        </Row>
      </Section>
    </>
  );
}

/** Edits locally and saves on blur or Enter, so typing never writes each key. */
function CommitInput(props: {
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
      <Textarea
        className="h-20 w-full resize-none"
        value={draft}
        placeholder={props.placeholder}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={commit}
      />
    );
  return (
    <Input
      className="w-full"
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

/** Order here is sidebar order. */
export const settingsPages: readonly SettingsPage[] = [
  {
    id: "general",
    label: "General",
    icon: FolderCog,
    rows: [
      "Start folder",
      "Default view",
      "Show hidden files",
      "Confirm before deleting",
    ],
    Component: GeneralSection,
  },
  {
    id: "cloud",
    label: "Cloud",
    icon: Cloud,
    rows: ["Accounts", "Add an account", "Provider", "Client secret", "Tenant"],
    Component: CloudAccountsSettings,
  },
  {
    id: "appearance",
    label: "Appearance",
    icon: Palette,
    rows: ["Compact rows", "Thumbnails"],
    Component: AppearanceSection,
  },
  {
    id: "search",
    label: "Search",
    icon: Search,
    rows: [
      "Index files automatically",
      "Index every",
      "Include hidden files",
      "Skip folders",
    ],
    Component: SearchSection,
  },
  {
    id: "shortcuts",
    label: "Shortcuts",
    icon: Keyboard,
    rows: ["Show shortcut hints"],
    Component: ShortcutsSection,
  },
  {
    id: "open-with",
    label: "Open with",
    icon: SquareArrowOutUpRight,
    rows: [],
    Component: OpenWithSection,
  },
  {
    id: "about",
    label: "About",
    icon: Info,
    breakBefore: true,
    rows: ["Version", "Settings file"],
    Component: AboutSection,
  },
];
