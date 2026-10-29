import { cloudRemotes, type CloudRemote } from "@/native/cloud";
import { errorText } from "@/shared/lib/format";
import { create } from "zustand";

/** The cloud accounts connected to Kura (each with its own OAuth app). */
export const useCloudStore = create<{
  remotes: CloudRemote[];
  loaded: boolean;
  error: string | null;
  load: () => Promise<void>;
}>((set) => ({
  remotes: [],
  loaded: false,
  error: null,
  load: async () => {
    try {
      set({ remotes: await cloudRemotes(), loaded: true, error: null });
    } catch (error) {
      set({ loaded: true, error: errorText(error) });
    }
  },
}));
