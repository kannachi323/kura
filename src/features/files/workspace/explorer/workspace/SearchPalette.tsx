import type { SearchResult } from "@/native/ipc";
import {
  Button,
  Command,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
  Dialog,
  DialogContent,
  DialogDescription,
  DialogTitle,
} from "@/shared/ui";
import { cn } from "@/shared/ui/utils";
import { useShallow } from "zustand/react/shallow";
import { useSearchStore } from "../../search";
import { SearchResultThumbnail } from "../components/SearchResultThumbnail";
import { revealSearchResultInPane } from "../utils/searchNavigation";

/**
 * Cmd+F: searches the on-device index by name. Choosing a result opens its
 * folder in the pane search started from and selects it.
 */
export function SearchPalette(props: { fallbackPaneId: string }) {
  const search = useSearchStore(
    useShallow((state) => ({
      open: state.open,
      query: state.query,
      scope: state.scope,
      results: state.results,
      searching: state.searching,
      status: state.status,
      error: state.error,
      paneId: state.paneId,
      currentPath: state.currentPath,
    })),
  );
  const store = useSearchStore.getState;
  const indexed = search.status?.indexedItemCount ?? 0;
  const scanning = Boolean(search.status?.scanInProgress);
  const trimmed = search.query.trim();

  const choose = (result: SearchResult) => {
    const entry = result.entry;
    const isFolder = entry.kind === "folder";
    const separator = Math.max(
      entry.path.lastIndexOf("/"),
      entry.path.lastIndexOf("\\"),
    );
    store().closeSearch();
    void revealSearchResultInPane(search.paneId ?? props.fallbackPaneId, {
      result,
      path: isFolder ? entry.path : entry.path.slice(0, separator) || "/",
      selectEntryId: isFolder ? null : entry.id,
    });
  };

  return (
    <Dialog
      open={search.open}
      onOpenChange={(open) => {
        if (!open) store().closeSearch();
      }}
    >
      <DialogContent
        placement="top"
        className="max-w-xl gap-0 overflow-hidden p-0 [&>[data-slot=dialog-close]]:hidden"
      >
        <DialogTitle className="sr-only">Search files</DialogTitle>
        <DialogDescription className="sr-only">
          Search file and folder names in Kura's on-device index.
        </DialogDescription>
        <Command shouldFilter={false} loop className="rounded-none">
          <CommandInput
            autoFocus
            value={search.query}
            placeholder="Search files and folders"
            onValueChange={(value) => store().setQuery(value)}
          />
          <div className="flex items-center gap-1 border-b border-charcoal-border px-3 py-1.5 text-xs">
            {(
              [
                ["everything", "Everywhere"],
                ["current", "This folder"],
              ] as const
            ).map(([scope, label]) => (
              <button
                key={scope}
                type="button"
                className={cn(
                  "rounded-md px-2 py-1 text-cream-muted outline-none hover:bg-control-hover hover:text-cream",
                  search.scope === scope && "bg-control-active text-cream",
                )}
                onClick={() => store().setScope(scope)}
              >
                {label}
              </button>
            ))}
          </div>
          <CommandList className="max-h-[min(420px,55vh)]">
            {trimmed && !search.searching ? (
              <CommandEmpty className="px-4 py-8 text-center text-sm text-cream-muted">
                {search.error ?? `No matches for “${trimmed}”.`}
              </CommandEmpty>
            ) : null}
            {!trimmed ? (
              <p className="px-4 py-8 text-center text-sm text-cream-muted">
                Type part of a name. Typos are fine.
              </p>
            ) : null}
            {search.results.map((result) => (
              <CommandItem
                key={result.entry.id}
                value={result.entry.id}
                onSelect={() => choose(result)}
                className="gap-2.5 rounded-md px-2.5 py-2"
              >
                <SearchResultThumbnail
                  result={result}
                  className="grid size-8 flex-none place-items-center overflow-hidden rounded bg-charcoal-hover text-cream-muted"
                  imageClassName="size-full object-cover"
                  iconSize={16}
                />
                <span className="grid min-w-0 gap-0.5">
                  <span className="truncate font-medium text-cream">
                    {result.entry.name}
                  </span>
                  <span className="truncate text-xs text-cream-muted">
                    {result.entry.path}
                  </span>
                </span>
              </CommandItem>
            ))}
          </CommandList>
          <footer className="flex min-h-10 items-center justify-between gap-3 border-t border-charcoal-border px-3 py-1.5 text-xs text-cream-muted">
            <span className="truncate">
              {scanning
                ? `Indexing… ${(search.status?.scanIndexedItemCount ?? 0).toLocaleString()} items so far`
                : indexed > 0
                  ? `${indexed.toLocaleString()} items indexed${search.searching ? " · searching…" : ""}`
                  : "Not indexed yet. Results come from recent and starred items."}
            </span>
            {scanning ? null : (
              <Button
                size="sm"
                variant="ghost"
                onClick={() => void store().startScan(search.currentPath)}
              >
                {indexed > 0 ? "Update index" : "Build index"}
              </Button>
            )}
          </footer>
        </Command>
      </DialogContent>
    </Dialog>
  );
}
