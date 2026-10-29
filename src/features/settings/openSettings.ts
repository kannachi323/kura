/** Fired on window to open Settings; the desktop frame owns the overlay. */
export const openSettingsEvent = "kura:open-settings";

let requestedSection: string | null = null;

export function requestOpenSettings() {
  window.dispatchEvent(new CustomEvent(openSettingsEvent));
}

/** Opens Settings at a section, such as "cloud". */
export function openSettingsAt(section: string) {
  requestedSection = section;
  requestOpenSettings();
}

/** The section the last `openSettingsAt` asked for, once. */
export function takeRequestedSettingsSection(): string | null {
  const section = requestedSection;
  requestedSection = null;
  return section;
}
