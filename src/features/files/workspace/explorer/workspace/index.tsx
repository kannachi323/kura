import { TransfersPage } from "../../../transfers/TransfersPage";
import { transfersPath } from "../../../transfers/transferModel";
import { useAppStore } from "@/features/app-shell";
import {
  selectAdvancedPreferences,
  selectFilePreferences,
  requestOpenSettings,
  selectGeneralPreferences,
  useSettingsStore,
} from "@/features/settings";
import { useTransientScrollbars } from "@/shared/hooks/useTransientScrollbars";
import { secondaryWindow } from "@/shared/platform/tauri";
import { appWindowSetFolder } from "../../native";
import { memo, useCallback, useEffect, useMemo, useRef } from "react";
import { useShallow } from "zustand/react/shallow";
import { ExplorerLoadingShell } from "../components/ExplorerLoadingShell";
import { ExplorerPane } from "../components/ExplorerPane";
import { openInNewTabEvent } from "../components/fileListKeyboard";
import { QuickLookHost } from "../components/QuickLookHost";
import { ExplorerSidebar } from "../components/ExplorerSidebar";
import { ExplorerDragProvider } from "../drag/ExplorerDragContext";
import { useExplorerStore } from "../store";
import { FileOperationConflictDialog } from "./FileOperationConflictDialog";
import { ExplorerDialog } from "./ExplorerBatchRenameDialog";
import { executableShortcutCommands } from "./ExplorerCommands";
import { filesMultiPanelStore } from "./explorerWorkspace/filesDockStores";
import { useFilesDockWorkspace } from "./explorerWorkspace/useFilesDockWorkspace";
import { CompareDialog } from "./ExplorerCompareDialog";
import { ExplorerContextMenu } from "./ExplorerContextMenu";
import {
  canCloseExplorerTab,
  canOpenTerminalPath,
  ExplorerTray,
  TransfersButton,
} from "./ExplorerDesktopPlugins";
import { cx } from "./ExplorerDesktopShared";
import {
  ExplorerNotifications,
  ExplorerRenameStatus,
} from "./ExplorerDesktopStatus";
import { DuplicateFinderDialog } from "./ExplorerDuplicateFinderDialog";
import { ExplorerMultiPanelWorkspace } from "./ExplorerMultiPanelWorkspace";
import { createExplorerAddTabControl } from "./ExplorerNewTabControl";
import { explorerShellStyles } from "./ExplorerShellStyles";
import {
  ConnectedExplorerToolbar,
  ConnectedFileInspector,
} from "./ExplorerToolbarConnections";
import { useExplorerDialogEvents } from "./explorerWorkspace/useExplorerDialogEvents";
import {
  useExplorerKeyboardShortcuts,
  useLegacyPluginTabMigration,
  useOperationErrorNotification,
} from "./explorerWorkspace/useExplorerWorkspaceEvents";
import { usePanelResize } from "./explorerWorkspace/usePanelResize";
import {
  useScopedExplorerWorkspace,
  type ExplorerWorkspaceProps,
} from "./explorerWorkspace/useScopedExplorerWorkspace";
import { useCloudEvents } from "./explorerWorkspace/useCloudEvents";
import { useTransferRefreshPolling } from "./explorerWorkspace/useTransferRefreshPolling";
import { resolveExplorerBottomBarRenderer } from "./ExplorerWorkspaceChrome";
import {
  buildExplorerLocationResults,
  resolveMountRoot,
  resolvePreferredWorkspaceRoot,
} from "./ExplorerWorkspaceUtils";
import { SearchPalette } from "./SearchPalette";
import { useDirectoryWatch } from "./useDirectoryWatch";
import { useSearchAutoIndex } from "./useSearchAutoIndex";
import { useExplorerDevices } from "./useExplorerDevices";
export type { ResizeTarget } from "../model/types/workspace/index";
export const ExplorerWorkspace = memo(function ExplorerWorkspace(
  props: ExplorerWorkspaceProps,
) {
  const multiPanelStore = useMemo(
    () => filesMultiPanelStore(props.workspaceId),
    [props.workspaceId],
  );
  const workspaceFocused = props.active !== false;
  const app = useAppStore((state) => state.app);
  const {
    sidebarWidth,
    previewWidth,
    pinnedPaths,
    library,
    operationError,
    inlineEdit,
    explorerDialogPaneId,
    contextMenuPaneId,
    notifications,
    pushNotification,
    dismissNotification,
  } = useExplorerStore(
    useShallow((state) => ({
      sidebarWidth: state.sidebarWidth,
      previewWidth: state.previewWidth,
      pinnedPaths: state.pinnedPaths,
      library: state.library,
      operationError: state.operationError,
      inlineEdit: state.inlineEdit,
      explorerDialogPaneId: state.dialog?.paneId ?? "",
      contextMenuPaneId: state.contextMenu.paneId,
      notifications: state.notifications,
      pushNotification: state.pushNotification,
      dismissNotification: state.dismissNotification,
    })),
  );
  const {
    activePaneId,
    activeTabPath,
    activeTabPreviewVisible,
    activeTabSidebarVisible,
    hasExplorerTabs,
    workspacePathSignature,
  } = multiPanelStore(
    useShallow((state) => {
      const activeTab =
        state.tabs.find((tab) => tab.id === state.activeTabId) ?? state.tabs[0];
      return {
        activePaneId: state.activePaneId,
        activeTabPath: activeTab?.path ?? "",
        activeTabPreviewVisible: activeTab?.previewVisible ?? true,
        activeTabSidebarVisible: activeTab?.sidebarVisible ?? true,
        hasExplorerTabs: state.tabs.length > 0,
        workspacePathSignature: state.tabs
          .flatMap((tab) => [tab.path, ...tab.panes.map((pane) => pane.path)])
          .join("\n"),
      };
    }),
  );
  const workspaceRef = useRef<HTMLElement | null>(null);
  useTransientScrollbars(workspaceRef);
  const mainRef = useRef<HTMLElement | null>(null);
  const { preferredWorkspaceRoot, settingsLoaded, settingsMountPath } =
    useSettingsStore(
      useShallow((state) => ({
        preferredWorkspaceRoot: selectGeneralPreferences(
          state.settings?.document,
        ).preferredWorkspaceRoot,
        settingsMountPath: selectAdvancedPreferences(state.settings?.document)
          .mountPath,
        settingsLoaded: state.loaded,
      })),
    );
  const filePreferences = useSettingsStore(
    useShallow((state) => selectFilePreferences(state.settings?.document)),
  );
  useEffect(() => {
    // Seeds the store-wide defaults a freshly opened pane inherits. Per-pane
    // overrides from the toolbar live in `paneViewModes`/`paneShowHidden` and
    // keep winning, so changing the default never yanks an open pane around.
    if (!settingsLoaded) return;
    useExplorerStore.setState({
      showHidden: filePreferences.showHiddenFiles,
      viewMode: filePreferences.defaultViewModeIndex === 1 ? "grid" : "list",
    });
  }, [
    filePreferences.defaultViewModeIndex,
    filePreferences.showHiddenFiles,
    settingsLoaded,
  ]);
  const environmentHomePath = app?.environment.homeDir ?? "/";
  const storageHomePath = resolvePreferredWorkspaceRoot(
    preferredWorkspaceRoot,
    environmentHomePath,
  );
  const homePath = storageHomePath;
  const { devicesLoading, mountedDevices, refreshDevices } =
    useExplorerDevices();
  useDirectoryWatch();
  useSearchAutoIndex(homePath);
  const mountRoot = resolveMountRoot(
    storageHomePath,
    settingsMountPath || app?.environment.mountPath || "",
  );
  const activePath = useExplorerStore((state) =>
    activeTabPath === transfersPath
      ? transfersPath
      : (state.panes[activePaneId]?.listing?.path ?? homePath),
  );
  const explorerInitialized = useExplorerStore((state) => state.initialized);
  const embeddedOpenInNewTab = useFilesDockWorkspace({
    workspaceId: props.workspaceId,
    activePath,
    initialized: explorerInitialized,
    homePath,
    multiPanelStore,
  });
  const openSidebarPathInNewTab = useCallback(
    (path: string, title?: string) => {
      if (props.embedded) embeddedOpenInNewTab(path, title);
      else
        multiPanelStore.getState().addTab(path, title || sidebarTabTitle(path));
    },
    [embeddedOpenInNewTab, multiPanelStore, props.embedded],
  );

  useEffect(() => {
    const open = (event: Event) => {
      const { path } = (event as CustomEvent<{ path: string }>).detail;
      openSidebarPathInNewTab(path);
    };
    window.addEventListener(openInNewTabEvent, open);
    return () => window.removeEventListener(openInNewTabEvent, open);
  }, [openSidebarPathInNewTab]);

  // A window opened with New Window starts at the folder it was opened for.
  const startPathApplied = useRef(false);
  useEffect(() => {
    const startPath = secondaryWindow()?.path;
    if (startPathApplied.current || !startPath || !activePaneId) return;
    startPathApplied.current = true;
    void useExplorerStore.getState().navigatePane(activePaneId, startPath);
  }, [activePaneId]);

  // An extra window reopens at its folder after a restart.
  useEffect(() => {
    if (secondaryWindow() && activePath)
      void appWindowSetFolder(activePath).catch(() => undefined);
  }, [activePath]);

  // Restore retired extension tabs to the built-in file browser.
  useLegacyPluginTabMigration({
    homePath,
    workspacePathSignature,
    multiPanelStore,
  });
  const activePaneIdRef = useRef(activePaneId);
  const activePathRef = useRef(activePath);
  const executableCommandIdsRef = useRef<readonly string[]>(
    executableShortcutCommands,
  );
  const ownsPane = useCallback(
    (paneId: string) =>
      multiPanelStore
        .getState()
        .tabs.some((tab) => tab.panes.some((pane) => pane.id === paneId)),
    [multiPanelStore],
  );
  const {
    duplicateFinderPaneId,
    setDuplicateFinderPaneId,
    compareDialog,
    setCompareDialog,
  } = useExplorerDialogEvents(activePaneIdRef, ownsPane);
  useTransferRefreshPolling(mountRoot);
  useCloudEvents();
  const {
    resizeTarget,
    resizeSidebarBy,
    resizePreviewBy,
    startSidebarResize,
    startPreviewResize,
  } = usePanelResize({
    workspaceRef,
    mainRef,
  });
  const workspacePaths = useMemo(
    () => workspacePathSignature.split("\n").filter(Boolean),
    [workspacePathSignature],
  );
  const locationResults = useMemo(
    () =>
      buildExplorerLocationResults(
        homePath,
        mountRoot,
        pinnedPaths,
        library,
        workspacePaths,
      ),
    [homePath, library, mountRoot, pinnedPaths, workspacePaths],
  );
  const sidebarVisible =
    activeTabSidebarVisible && activePath !== transfersPath;
  const previewVisible =
    activeTabPreviewVisible && activePath !== transfersPath;
  useEffect(() => {
    activePaneIdRef.current = activePaneId;
    activePathRef.current = activePath;
  }, [activePaneId, activePath]);
  useScopedExplorerWorkspace(props, homePath, settingsLoaded);
  useOperationErrorNotification(operationError, pushNotification);
  useExplorerKeyboardShortcuts({
    active: props.active,
    executableCommandIdsRef,
    multiPanelStore,
  });
  const navigateSidebar = useCallback(
    (path: string) => {
      const paneId = multiPanelStore.getState().activePaneId;
      if (paneId) void useExplorerStore.getState().navigatePane(paneId, path);
    },
    [multiPanelStore],
  );
  const renderToolbar = useCallback(
    (paneId: string, path: string) => {
      if (path === transfersPath) return null;
      return (
        <ConnectedExplorerToolbar
          paneId={paneId}
          fallbackPath={path}
          locationResults={locationResults}
          searchActions={props.embedded ? <TransfersButton /> : null}
          extraActions={
            props.embedded ? (
              <ExplorerTray
                terminalEnabled={canOpenTerminalPath(path)}
                terminalPath={path}
              />
            ) : null
          }
        />
      );
    },
    [locationResults, props.embedded],
  );
  const renderPane = useCallback(
    (paneId: string, path: string) => {
      if (path === transfersPath) return <TransfersPage />;
      return (
        <ExplorerPane
          paneId={paneId}
          path={path}
          isActive={activePaneId === paneId}
        />
      );
    },
    [activePaneId],
  );
  const inspector = previewVisible ? (
    <ConnectedFileInspector paneId={activePaneId} />
  ) : undefined;
  const explorerSidebar = useMemo(
    () =>
      sidebarVisible ? (
        <ExplorerSidebar
          homePath={homePath}
          activePath={activePath}
          mountRoot={mountRoot}
          library={library}
          devices={mountedDevices}
          devicesLoading={devicesLoading}
          pinnedPaths={pinnedPaths}
          onNavigate={navigateSidebar}
          onRefreshDevices={refreshDevices}
          onOpenInNewTab={openSidebarPathInNewTab}
          onUnpinPinnedPath={useExplorerStore.getState().togglePinnedPath}
        />
      ) : undefined,
    [
      activePath,
      devicesLoading,
      homePath,
      library,
      mountRoot,
      mountedDevices,
      navigateSidebar,
      openSidebarPathInNewTab,
      pinnedPaths,
      refreshDevices,
      sidebarVisible,
    ],
  );
  const renderTabActions = useCallback(
    () => (
      <ExplorerTray
        onOpenSettings={requestOpenSettings}
        terminalEnabled={
          canOpenTerminalPath(activeTabPath) && canOpenTerminalPath(activePath)
        }
        terminalPath={activePath}
      />
    ),
    [activePath, activeTabPath],
  );
  const renderAddTabControl = useMemo(
    () => createExplorerAddTabControl(homePath),
    [homePath],
  );
  if (!explorerInitialized || !hasExplorerTabs)
    return <ExplorerLoadingShell embedded={props.embedded} />;
  return (
    <ExplorerDragProvider>
      <section
        ref={workspaceRef}
        className={cx(
          explorerShellStyles.workspaceBase,
          !sidebarVisible && explorerShellStyles.workspaceCollapsed,
        )}
      >
        <main ref={mainRef} className={explorerShellStyles.main}>
          <ExplorerMultiPanelWorkspace
            store={multiPanelStore}
            className="explorer-multipanel"
            canCloseTab={(tab) =>
              canCloseExplorerTab(tab, multiPanelStore.getState().tabs)
            }
            renderBottomBar={resolveExplorerBottomBarRenderer(props.embedded)}
            renderAddTabControl={renderAddTabControl}
            renderTabActions={renderTabActions}
            renderToolbar={renderToolbar}
            showTabStrip={!props.embedded}
            renderNavigationAside={explorerSidebar}
            navigationAsideWidth={sidebarWidth}
            onNavigationAsideResizeStart={startSidebarResize}
            onNavigationAsideResizeBy={resizeSidebarBy}
            navigationAsideResizing={resizeTarget === "sidebar"}
            renderAside={activePath === transfersPath ? undefined : inspector}
            asideWidth={previewWidth}
            onAsideResizeStart={startPreviewResize}
            onAsideResizeBy={resizePreviewBy}
            asideResizing={resizeTarget === "preview"}
            renderPane={renderPane}
          />
        </main>
        {workspaceFocused ? <FileOperationConflictDialog /> : null}
        {workspaceFocused ? (
          <>
            <SearchPalette fallbackPaneId={activePaneId} />
            <QuickLookHost />
          </>
        ) : null}
        {inlineEdit && ownsPane(inlineEdit.paneId) ? (
          <ExplorerRenameStatus edit={inlineEdit} />
        ) : null}
        {workspaceFocused ? (
          <ExplorerNotifications
            notifications={notifications}
            onDismiss={dismissNotification}
          />
        ) : null}
        {duplicateFinderPaneId ? (
          <DuplicateFinderDialog
            paneId={duplicateFinderPaneId}
            defaultRoot={
              useExplorerStore.getState().panes[duplicateFinderPaneId]?.listing
                ?.path ?? activePath
            }
            onClose={() => setDuplicateFinderPaneId(null)}
          />
        ) : null}
        {compareDialog ? (
          <CompareDialog
            seed={compareDialog}
            onClose={() => setCompareDialog(null)}
          />
        ) : null}
        {ownsPane(contextMenuPaneId) ? <ExplorerContextMenu /> : null}
        {explorerDialogPaneId && ownsPane(explorerDialogPaneId) ? (
          <ExplorerDialog />
        ) : null}
      </section>
    </ExplorerDragProvider>
  );
});
export default ExplorerWorkspace;

function sidebarTabTitle(path: string): string {
  if (path === transfersPath) return "Transfers";
  return path.replace(/\/+$/, "").split("/").filter(Boolean).pop() ?? "Files";
}
