type SettingsDocument = Record<string, unknown> | null | undefined;

function sectionRecord(document: Record<string, unknown>, section: string) {
  const value = document[section];
  return value && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};
}

export function settingsBoolean(
  document: Record<string, unknown>,
  section: string,
  key: string,
  fallback: boolean,
): boolean {
  const value = sectionRecord(document, section)[key];
  return typeof value === "boolean" ? value : fallback;
}

export function settingsNumber(
  document: Record<string, unknown>,
  section: string,
  key: string,
  fallback: number,
): number {
  const value = sectionRecord(document, section)[key];
  return typeof value === "number" && Number.isFinite(value) ? value : fallback;
}

export function settingsString(
  document: Record<string, unknown>,
  section: string,
  key: string,
  fallback: string,
): string {
  const value = sectionRecord(document, section)[key];
  return typeof value === "string" ? value : fallback;
}

export function selectAppearancePreferences(
  document: SettingsDocument,
): AppearancePreferences {
  const source = document ?? {};
  const theme = settingsString(source, "appearance", "theme", "system");
  return {
    theme: theme === "light" || theme === "dark" ? theme : "system",
    compactModeEnabled: settingsBoolean(
      source,
      "appearance",
      "compact_mode_enabled",
      false,
    ),
    thumbnailPreviewsEnabled: settingsBoolean(
      source,
      "appearance",
      "thumbnail_previews_enabled",
      true,
    ),
  };
}

export function selectFilePreferences(
  document: SettingsDocument,
): FilePreferences {
  const source = document ?? {};
  return {
    defaultViewModeIndex: settingsNumber(
      source,
      "files",
      "default_view_mode_index",
      0,
    ),
    showHiddenFiles: settingsBoolean(
      source,
      "files",
      "show_hidden_files",
      false,
    ),
  };
}

export function selectNotificationPreferences(
  document: SettingsDocument,
): NotificationPreferences {
  const source = document ?? {};
  return {
    desktopNotificationsEnabled: settingsBoolean(
      source,
      "notifications",
      "desktop_notifications_enabled",
      true,
    ),
  };
}

export function selectGeneralPreferences(
  document: SettingsDocument,
): GeneralPreferences {
  const source = document ?? {};
  return {
    confirmDestructiveActions: settingsBoolean(
      source,
      "general",
      "confirm_destructive_actions",
      true,
    ),
    defaultFileActionIndex: settingsNumber(
      source,
      "general",
      "default_file_action_index",
      0,
    ),
    preferredWorkspaceRoot: settingsString(
      source,
      "general",
      "preferred_workspace_root",
      "",
    ),
  };
}

export function selectShortcutPreferences(
  document: SettingsDocument,
): ShortcutPreferences {
  const source = document ?? {};
  return {
    shortcutHintsEnabled: settingsBoolean(
      source,
      "shortcuts",
      "shortcut_hints_enabled",
      true,
    ),
  };
}

export function selectAdvancedPreferences(
  document: SettingsDocument,
): AdvancedPreferences {
  const source = document ?? {};
  return { mountPath: settingsString(source, "advanced", "mount_path", "") };
}

export function selectSearchMaintenancePreferences(
  document: SettingsDocument,
): SearchMaintenancePreferences {
  const source = document ?? {};
  return {
    automaticFileDiscoveryEnabled: settingsBoolean(
      source,
      "search",
      "automatic_file_discovery_enabled",
      true,
    ),
    discoveryIntervalMinutes: clamp(
      settingsNumber(source, "search", "discovery_interval_minutes", 15),
      5,
      240,
    ),
    ignoredPaths: parsePathList(
      settingsString(source, "search", "ignored_paths", ""),
    ),
    includeHidden: settingsBoolean(source, "search", "include_hidden", false),
    maxDepth: clamp(settingsNumber(source, "search", "max_depth", 18), 1, 64),
  };
}

/** Newline- or comma-separated in the settings field, a list everywhere else. */
export function parsePathList(value: string): string[] {
  return value
    .split(/[\n,]/)
    .map((entry) => entry.trim())
    .filter(Boolean);
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export type AppTheme = "system" | "light" | "dark";
export interface AppearancePreferences {
  theme: AppTheme;
  compactModeEnabled: boolean;
  thumbnailPreviewsEnabled: boolean;
}
export interface FilePreferences {
  defaultViewModeIndex: number;
  showHiddenFiles: boolean;
}
export interface NotificationPreferences {
  desktopNotificationsEnabled: boolean;
}
export interface GeneralPreferences {
  confirmDestructiveActions: boolean;
  defaultFileActionIndex: number;
  preferredWorkspaceRoot: string;
}
export interface ShortcutPreferences {
  shortcutHintsEnabled: boolean;
}
export interface AdvancedPreferences {
  mountPath: string;
}
export interface SearchMaintenancePreferences {
  automaticFileDiscoveryEnabled: boolean;
  discoveryIntervalMinutes: number;
  ignoredPaths: string[];
  includeHidden: boolean;
  maxDepth: number;
}
