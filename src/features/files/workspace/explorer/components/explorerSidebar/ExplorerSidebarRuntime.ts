import type { ComponentProps, ComponentType } from "react";
import type { ExplorerDropTarget } from "../../drag/ExplorerDropTarget";
import {
  type PreviewErrorComponent,
  type SidebarDeviceEntry,
} from "@/features/file-ui";
import type { useSidebarPreferences } from "./useSidebarPreferences";
import type { useSidebarQuickAccess } from "./useSidebarQuickAccess";
import type { useSidebarSmartFolders } from "./useSidebarSmartFolders";
export interface ExplorerSidebarRuntime {
  Error: PreviewErrorComponent;
  DropTarget: ComponentType<ComponentProps<typeof ExplorerDropTarget>>;
  useSidebarPreferences: typeof useSidebarPreferences;
  useSidebarQuickAccess: typeof useSidebarQuickAccess;
  useSidebarSmartFolders: typeof useSidebarSmartFolders;
  confirmUnmount(message: string): Promise<boolean>;
  unmount(device: SidebarDeviceEntry): Promise<unknown>;
}
