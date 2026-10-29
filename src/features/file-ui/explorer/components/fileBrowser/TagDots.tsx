import type { FileEntry, FinderTag } from "@/native/ipc";
import { useFileBrowserRuntime } from "./FileBrowserRuntime";

/** Finder's tag colours, by colour index. */
export const tagColors: Record<number, string> = {
  1: "#8e8e93",
  2: "#34c759",
  3: "#af52de",
  4: "#0a84ff",
  5: "#ffcc00",
  6: "#ff3b30",
  7: "#ff9500",
};

/** An entry's Finder tags as colour dots, when the runtime provides tags. */
export function EntryTagDots(props: { entry: FileEntry }) {
  const useEntryTags = useFileBrowserRuntime().useEntryTags ?? noTags;
  return <TagDots tags={useEntryTags(props.entry)} />;
}

function noTags(): undefined {
  return undefined;
}

/** Overlapping colour dots for an item's tags, as Finder shows them. */
export function TagDots(props: { tags: FinderTag[] | undefined }) {
  const colored = (props.tags ?? []).filter((tag) => tagColors[tag.color]);
  if (colored.length === 0) return null;
  return (
    <span
      className="flex flex-none items-center"
      title={(props.tags ?? []).map((tag) => tag.name).join(", ")}
    >
      {colored.slice(0, 3).map((tag, index) => (
        <span
          key={tag.name}
          aria-hidden="true"
          className="size-2.5 rounded-full ring-[1.5px] ring-charcoal-bg"
          style={{
            backgroundColor: tagColors[tag.color],
            marginLeft: index === 0 ? 0 : -4,
          }}
        />
      ))}
      <span className="sr-only">
        Tags: {(props.tags ?? []).map((tag) => tag.name).join(", ")}
      </span>
    </span>
  );
}
