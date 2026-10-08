import { appSnapshot } from "@/native";
import type { AppSnapshot } from "@/native/ipc";
import { create } from "zustand";

export const useAppStore = create<AppStore>((set) => ({
  app: null,
  loadApp: async () => {
    try {
      set({ app: await appSnapshot() });
    } catch {
      set({ app: null });
    }
  },
}));

export interface AppStore {
  app: AppSnapshot | null;
  loadApp: () => Promise<void>;
}
