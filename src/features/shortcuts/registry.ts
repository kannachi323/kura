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
    description: "Open the current folder in a new tab.",
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
