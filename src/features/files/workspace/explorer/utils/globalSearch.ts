export type { ExplorerSearchOptions } from "../model/interfaces/utils/globalSearch";
export { isPathWithin } from "./globalSearch/searchPaths";
import { searchQuery } from "../../native";
import type {
  ExplorerLibrarySnapshot,
  SearchQueryRequest,
  SearchResult,
} from "@/native/ipc";
import type { ExplorerSearchOptions } from "../model/interfaces/utils/globalSearch";
import { mergeLibrarySearchResults } from "./librarySearch";
import {
  selectSearchMaintenancePreferences,
  useSettingsStore,
} from "@/features/settings";

/** Searches Kura's local index. Nothing leaves the device. */
export async function queryIndexedExplorerSearch(
  query: string,
  options: ExplorerSearchOptions,
  library: ExplorerLibrarySnapshot | null,
): Promise<SearchResult[]> {
  const request: SearchQueryRequest = {
    query,
    currentPath: options.currentPath,
    scope: options.scope,
    includeFiles: options.includeFiles ?? true,
    includeDirectories: options.includeDirectories ?? true,
    includeHidden:
      options.includeHidden ??
      selectSearchMaintenancePreferences(
        useSettingsStore.getState().settings?.document,
      ).includeHidden,
    limit: options.limit ?? 100,
    rules: options.rules,
    matchMode: options.matchMode,
  };
  const results = await searchQuery(request);
  return mergeLibrarySearchResults(results, library, query, options);
}
