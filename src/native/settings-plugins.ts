import type {
  OpenWithAssociation,
  SaveSettingsRequest,
  SettingsSnapshot,
} from "@/native/ipc";

import { invoke } from "./invoke";
export function settingsSnapshot(): Promise<SettingsSnapshot> {
  return invoke("settings_snapshot");
}

export function settingsSave(
  request: SaveSettingsRequest,
): Promise<SettingsSnapshot> {
  return invoke("settings_save", { request });
}

export function settingsOpenWithAssociations(): Promise<OpenWithAssociation[]> {
  return invoke("settings_open_with_associations");
}
