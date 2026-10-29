import {
  mergeLibrarySearchResults,
  queryIndexedExplorerSearch,
  useExplorerStore,
} from "../../explorer";
import {
  searchCancelScan,
  searchGetStatus,
  searchInit,
  searchStartScan,
} from "../../native";
import {
  selectSearchMaintenancePreferences,
  useSettingsStore,
} from "@/features/settings";
import type { SearchResult, SearchStatus } from "@/native/ipc";
import type { SearchQueryScope } from "@/native/ipc/primitives";
import { userFacingErrorText } from "@/shared/lib/format";
import { create } from "zustand";

const searchDebounceMs = 180;
const activeStatusPollMs = 500;
const idleStatusPollMs = 5000;

let debounceTimer: number | null = null;
let statusPollTimer: number | null = null;
let querySequence = 0;
let indexedResults: SearchResult[] = [];
let indexedError: string | null = null;

export const useSearchStore = create<SearchStore>((set, get) => ({
  open: false,
  paneId: null,
  query: "",
  scope: "everything",
  currentPath: "",
  results: [],
  searching: false,
  status: null,
  error: null,
  initialized: false,
  initialize: async () => {
    try {
      const status = get().initialized
        ? await searchGetStatus()
        : await searchInit();
      set({ initialized: true, status });
    } catch (error) {
      set({ initialized: true, error: userFacingErrorText(error) });
    }
  },
  openSearch: async (currentPath, paneId = null) => {
    set({ open: true, currentPath, paneId, error: null });
    await get().initialize();
    scheduleStatusPolling();
    scheduleSearch();
  },
  closeSearch: () => {
    clearSearchDebounce();
    querySequence += 1;
    set({ open: false, searching: false });
    stopStatusPolling();
  },
  setQuery: (query) => {
    set({ query });
    scheduleSearch();
  },
  setScope: (scope) => {
    set({ scope });
    scheduleSearch(0);
  },
  refreshStatus: async () => {
    try {
      const status = await searchGetStatus();
      set({ status, error: null });
    } catch (error) {
      set({ error: userFacingErrorText(error) });
    }
  },
  startScan: async (currentPath) => {
    try {
      const searchPreferences = selectSearchMaintenancePreferences(
        useSettingsStore.getState().settings?.document,
      );
      const status = await searchStartScan({
        includeLocal: true,
        includeRemotes: false,
        roots: [],
        maxDepth: searchPreferences.maxDepth,
        ignoredPaths: searchPreferences.ignoredPaths,
        incremental: true,
        // Live updates between scans follow "Index files automatically".
        watch: searchPreferences.automaticFileDiscoveryEnabled,
      });
      set({ status, currentPath, error: null });
      scheduleStatusPolling();
    } catch (error) {
      set({ error: userFacingErrorText(error) });
    }
  },
  cancelScan: async () => {
    try {
      const status = await searchCancelScan();
      set({ status, error: null });
      scheduleStatusPolling();
    } catch (error) {
      set({ error: userFacingErrorText(error) });
    }
  },
  executeSearch: async () => {
    clearSearchDebounce();
    const { query } = get();
    const trimmed = query.trim();
    const sequence = ++querySequence;
    if (!trimmed) {
      indexedResults = [];
      indexedError = null;
      set({ results: [], searching: false });
      return;
    }
    set({ searching: true, error: null });
    await executeIndexedSearch(sequence);
  },
}));

function scheduleSearch(delay = searchDebounceMs): void {
  clearSearchDebounce();
  const sequence = ++querySequence;
  indexedResults = [];
  indexedError = null;
  debounceTimer = window.setTimeout(() => {
    void executeIndexedSearch(sequence);
  }, delay);
}

function clearSearchDebounce(): void {
  if (debounceTimer !== null) {
    window.clearTimeout(debounceTimer);
    debounceTimer = null;
  }
}

async function executeIndexedSearch(sequence: number): Promise<void> {
  const { query, scope, currentPath, status } = useSearchStore.getState();
  const trimmed = query.trim();
  if (!trimmed || sequence !== querySequence) return;
  useSearchStore.setState({ searching: true, error: null });
  let nextIndexedResults: SearchResult[];
  let nextIndexedError: string | null = null;
  try {
    nextIndexedResults =
      !status || status.indexedItemCount === 0
        ? mergeLibrarySearchResults(
            [],
            useExplorerStore.getState().library,
            trimmed,
            {
              scope,
              currentPath,
              limit: 100,
            },
          )
        : await queryIndexedExplorerSearch(
            trimmed,
            { scope, currentPath, limit: 100 },
            useExplorerStore.getState().library,
          );
  } catch (error) {
    nextIndexedResults = mergeLibrarySearchResults(
      [],
      useExplorerStore.getState().library,
      trimmed,
      {
        scope,
        currentPath,
        limit: 100,
      },
    );
    nextIndexedError = userFacingErrorText(error);
  }
  if (sequence !== querySequence) return;
  indexedResults = nextIndexedResults;
  indexedError = nextIndexedError;
  useSearchStore.setState({
    results: indexedResults,
    searching: false,
    error: indexedResults.length > 0 ? null : indexedError,
  });
}

function scheduleStatusPolling(): void {
  stopStatusPolling();
  const poll = async () => {
    await useSearchStore.getState().refreshStatus();
    const status = useSearchStore.getState().status;
    const active = Boolean(status?.scanInProgress);
    if (useSearchStore.getState().open || active) {
      statusPollTimer = window.setTimeout(
        poll,
        active ? activeStatusPollMs : idleStatusPollMs,
      );
    }
  };
  statusPollTimer = window.setTimeout(poll, activeStatusPollMs);
}

function stopStatusPolling(): void {
  if (statusPollTimer !== null) {
    window.clearTimeout(statusPollTimer);
    statusPollTimer = null;
  }
}

export interface SearchStore {
  open: boolean;
  paneId: string | null;
  query: string;
  scope: SearchQueryScope;
  currentPath: string;
  results: SearchResult[];
  searching: boolean;
  status: SearchStatus | null;
  error: string | null;
  initialized: boolean;
  initialize: () => Promise<void>;
  /** Opens search for a folder; results open in `paneId` (or the active pane). */
  openSearch: (currentPath: string, paneId?: string | null) => Promise<void>;
  closeSearch: () => void;
  setQuery: (query: string) => void;
  setScope: (scope: SearchQueryScope) => void;
  refreshStatus: () => Promise<void>;
  startScan: (currentPath: string) => Promise<void>;
  cancelScan: () => Promise<void>;
  executeSearch: () => Promise<void>;
}
