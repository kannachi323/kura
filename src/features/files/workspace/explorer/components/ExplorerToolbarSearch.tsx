import { SystemErrorActivity } from "@/features/activity";
import { useSearchStore } from "../../search";
import { useExplorerStore } from "../store";
import { queryIndexedExplorerSearch } from "../utils/globalSearch";
import { SearchResultThumbnail } from "./SearchResultThumbnail";
import {
  ExplorerToolbarSearchView,
  type ExplorerToolbarSearchProps,
  type ExplorerToolbarSearchRuntime,
} from "./ExplorerToolbarSearchView";
export type { ExplorerToolbarSearchProps } from "./ExplorerToolbarSearchView";
const runtime: ExplorerToolbarSearchRuntime = {
  Error: SystemErrorActivity,
  Thumbnail: SearchResultThumbnail,
  openSearch: (path) => {
    void useSearchStore.getState().openSearch(path);
  },
  async query(query, currentPath, signal) {
    const options = { scope: "everything" as const, currentPath, limit: 8 };
    const results = await queryIndexedExplorerSearch(
      query,
      options,
      useExplorerStore.getState().library,
    );
    if (signal.aborted)
      throw new DOMException("Search cancelled", "AbortError");
    return results;
  },
};
export function ExplorerToolbarSearch(props: ExplorerToolbarSearchProps) {
  return <ExplorerToolbarSearchView {...props} runtime={runtime} />;
}
