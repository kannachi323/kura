import type {
  ShortcutBindingPair,
  ShortcutCommandDefinition,
  ShortcutPlatform,
} from "./types";
import { command } from "./factory";
import { filesShortcutCommands } from "./filesCommands";
export type * from "./types";

const appCommands: ShortcutCommandDefinition[] = [
  command("app.open_settings", "Open settings", {
    description: "Open Kura settings.",
    category: "Kura",
    aliases: ["preferences", "configuration"],
    mac: "Cmd+Comma",
    windows: "Ctrl+Comma",
    allowInEditable: true,
    nativeMenu: true,
  }),
  command("app.zoom_in", "Zoom in", {
    description: "Increase the interface scale.",
    category: "View",
    mac: "Cmd+Plus",
    macAlternate: "Ctrl+Plus",
    windows: "Ctrl+Plus",
    allowInEditable: true,
    repeatable: true,
    nativeMenu: true,
  }),
  command("app.zoom_out", "Zoom out", {
    description: "Decrease the interface scale.",
    category: "View",
    mac: "Cmd+Minus",
    macAlternate: "Ctrl+Minus",
    windows: "Ctrl+Minus",
    allowInEditable: true,
    repeatable: true,
    nativeMenu: true,
  }),
  command("app.zoom_reset", "Reset zoom", {
    description: "Return the interface to its default scale.",
    category: "View",
    mac: "Cmd+0",
    macAlternate: "Ctrl+0",
    windows: "Ctrl+0",
    allowInEditable: true,
    nativeMenu: true,
  }),
  command("navigation.back", "Go back", {
    description: "Go back to the previous folder.",
    category: "Navigation",
    mac: "Cmd+LeftBracket",
    windows: "Alt+ArrowLeft",
    allowInEditable: true,
    repeatable: true,
  }),
  command("navigation.forward", "Go forward", {
    description: "Go forward to the next folder.",
    category: "Navigation",
    mac: "Cmd+RightBracket",
    windows: "Alt+ArrowRight",
    allowInEditable: true,
    repeatable: true,
  }),
];

const tabCommands: ShortcutCommandDefinition[] = [
  command("explorer.new_tab", "New tab", {
    description: "Open a new tab.",
    category: "Tabs",
    mac: "Cmd+T",
    windows: "Ctrl+T",
    allowInEditable: true,
    nativeMenu: true,
  }),
  command("explorer.close_tab", "Close tab", {
    description: "Close the active tab.",
    category: "Tabs",
    mac: "Cmd+W",
    windows: "Ctrl+W",
    allowInEditable: true,
    nativeMenu: true,
  }),
  command("explorer.restore_tab", "Reopen closed tab", {
    description: "Restore the most recently closed tab.",
    category: "Tabs",
    mac: "Cmd+Shift+T",
    windows: "Ctrl+Shift+T",
    allowInEditable: true,
    nativeMenu: true,
  }),
  command("explorer.next_workspace", "Next tab", {
    description: "Switch to the next tab.",
    category: "Tabs",
    mac: "Ctrl+Tab",
    windows: "Ctrl+Tab",
    allowInEditable: true,
    repeatable: true,
  }),
  command("workspace.previous_tab", "Previous tab", {
    description: "Switch to the previous tab.",
    category: "Tabs",
    mac: "Ctrl+Shift+Tab",
    windows: "Ctrl+Shift+Tab",
    allowInEditable: true,
    repeatable: true,
  }),
  command("workspace.split_right", "Split pane right", {
    description: "Open a new pane beside the focused one.",
    category: "Panes",
    mac: "Cmd+Backslash",
    windows: "Ctrl+Backslash",
    allowInEditable: true,
  }),
  command("workspace.split_down", "Split pane below", {
    description: "Open a new pane below the focused one.",
    category: "Panes",
    mac: "Cmd+Shift+Backslash",
    windows: "Ctrl+Shift+Backslash",
    allowInEditable: true,
  }),
  command("workspace.close_pane", "Close focused pane", {
    description: "Close the focused pane and keep the others.",
    category: "Panes",
  }),
  command("workspace.focus_pane_left", "Focus pane left", {
    description: "Move focus to the pane on the left.",
    category: "Panes",
    mac: "Option+H",
    windows: "Alt+H",
  }),
  command("workspace.focus_pane_right", "Focus pane right", {
    description: "Move focus to the pane on the right.",
    category: "Panes",
    mac: "Option+L",
    windows: "Alt+L",
  }),
  command("workspace.focus_pane_up", "Focus pane above", {
    description: "Move focus to the pane above.",
    category: "Panes",
    mac: "Option+K",
    windows: "Alt+K",
  }),
  command("workspace.focus_pane_down", "Focus pane below", {
    description: "Move focus to the pane below.",
    category: "Panes",
    mac: "Option+J",
    windows: "Alt+J",
  }),
  ...Array.from({ length: 9 }, (_, index) =>
    command(`explorer.tab_${index + 1}`, `Go to tab ${index + 1}`, {
      description: `Switch to tab ${index + 1}.`,
      category: "Tabs",
      mac: `Cmd+${index + 1}`,
      windows: `Ctrl+${index + 1}`,
      allowInEditable: true,
    }),
  ),
];

export const shortcutCommandRegistry: ShortcutCommandDefinition[] = [
  ...appCommands,
  ...tabCommands,
  ...filesShortcutCommands,
];

export const shortcutCommandsById = new Map(
  shortcutCommandRegistry.map((definition) => [definition.id, definition]),
);

export function defaultBindingsFor(
  definition: ShortcutCommandDefinition,
  platform: ShortcutPlatform,
): ShortcutBindingPair {
  return definition.defaults[platform];
}
