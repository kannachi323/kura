import { SystemErrorActivity } from "@/features/activity";
import { devicesUnmount } from "../../../native";
import { ExplorerDropTarget } from "../../drag/ExplorerDropTarget";
import { useSidebarPreferences } from "./useSidebarPreferences";
import { useSidebarQuickAccess } from "./useSidebarQuickAccess";
import { useSidebarSmartFolders } from "./useSidebarSmartFolders";
import type { ExplorerSidebarRuntime } from "./ExplorerSidebarRuntime";
export const hostExplorerSidebarRuntime: ExplorerSidebarRuntime = {
  Error: SystemErrorActivity,
  DropTarget: ExplorerDropTarget,
  useSidebarPreferences,
  useSidebarQuickAccess,
  useSidebarSmartFolders,
  confirmUnmount: async (message) => window.confirm(message),
  unmount: (device) =>
    devicesUnmount({ volumeId: device.volumeId, mountPath: device.mountPath }),
};
