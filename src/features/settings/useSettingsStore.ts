import { settingsSave, settingsSnapshot } from "@/native";
import { errorText } from "@/shared/lib/format";
import { create } from "zustand";

export type SettingValue = string | number | boolean | Record<string, string>;

export interface KuraSettings {
  document: Record<string, unknown>;
}

let settingsLoad: Promise<void> | null = null;
let writeQueue: Promise<unknown> = Promise.resolve();

/** Kura keeps its settings in one local JSON document owned by the native side. */
export const useSettingsStore = create<SettingsStore>((set, get) => ({
  settings: null,
  loaded: false,
  error: null,
  load: async () => {
    if (settingsLoad) return settingsLoad;
    settingsLoad = (async () => {
      try {
        const snapshot = await settingsSnapshot();
        set({
          settings: { document: snapshot.document },
          loaded: true,
          error: null,
        });
      } catch (error) {
        set({
          settings: { document: {} },
          loaded: true,
          error: errorText(error),
        });
      } finally {
        settingsLoad = null;
      }
    })();
    return settingsLoad;
  },
  update: async (section, key, value) => {
    const current = get().settings?.document ?? {};
    const sectionValue = current[section];
    const document = {
      ...current,
      [section]: {
        ...(sectionValue && typeof sectionValue === "object"
          ? sectionValue
          : {}),
        [key]: value,
      },
    };
    set({ settings: { document } });
    await persist(document, set);
  },
  setOpenWithAssociation: async (filePath, applicationPath) => {
    const name =
      filePath.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? filePath;
    const dot = name.lastIndexOf(".");
    const key = (dot >= 0 ? name.slice(dot) : name).toLowerCase();
    const document = {
      ...(get().settings?.document ?? {}),
      open_with: {
        ...openWith(get().settings?.document),
        [key]: applicationPath,
      },
    };
    set({ settings: { document } });
    await persist(document, set);
  },
  removeOpenWithAssociation: async (key) => {
    const associations = openWith(get().settings?.document);
    delete associations[key];
    const document = {
      ...(get().settings?.document ?? {}),
      open_with: associations,
    };
    set({ settings: { document } });
    await persist(document, set);
  },
}));

function openWith(
  document: Record<string, unknown> | undefined,
): Record<string, string> {
  const value = document?.open_with;
  return value && typeof value === "object"
    ? { ...(value as Record<string, string>) }
    : {};
}

function persist(
  document: Record<string, unknown>,
  set: (partial: Partial<SettingsStore>) => void,
): Promise<unknown> {
  writeQueue = writeQueue
    .catch(() => undefined)
    .then(() => settingsSave({ document }))
    .then(() => set({ error: null }))
    .catch((error) => set({ error: errorText(error) }));
  return writeQueue;
}

export interface SettingsStore {
  settings: KuraSettings | null;
  loaded: boolean;
  error: string | null;
  load: () => Promise<void>;
  update: (section: string, key: string, value: SettingValue) => Promise<void>;
  setOpenWithAssociation: (
    filePath: string,
    applicationPath: string,
  ) => Promise<void>;
  removeOpenWithAssociation: (key: string) => Promise<void>;
}
