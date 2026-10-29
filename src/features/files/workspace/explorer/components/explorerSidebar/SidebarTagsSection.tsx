import {
  SidebarSectionHeader,
  sidebarStyles,
  tagColors,
} from "@/features/file-ui";
import { Button, cn, Collapsible, CollapsibleContent } from "@/shared/ui";

/** Finder's colour tags, in its sidebar order, with their colour indexes. */
const sidebarTags = [
  ["Red", 6],
  ["Orange", 7],
  ["Yellow", 5],
  ["Green", 2],
  ["Blue", 4],
  ["Purple", 3],
  ["Gray", 1],
] as const;

export function tagLocation(name: string): string {
  return `kura://tag/${encodeURIComponent(name)}`;
}

/** Finder's Tags section: each tag opens the items that carry it. */
export function SidebarTagsSection(props: {
  activePath: string;
  collapsed: boolean;
  onToggle: () => void;
  onNavigate: (path: string) => void;
}) {
  return (
    <Collapsible className={sidebarStyles.section} open={!props.collapsed}>
      <SidebarSectionHeader
        title="Tags"
        collapsed={props.collapsed}
        onToggle={props.onToggle}
      />
      <CollapsibleContent className={sidebarStyles.list}>
        {sidebarTags.map(([name, color]) => {
          const path = tagLocation(name);
          return (
            <div
              key={name}
              className={cn(
                sidebarStyles.treeSurface,
                sidebarStyles.pinnedRow,
                props.activePath === path && sidebarStyles.itemSelected,
              )}
            >
              <Button
                type="button"
                variant="ghost"
                className={sidebarStyles.pinnedButton}
                onClick={() => props.onNavigate(path)}
              >
                <span className={sidebarStyles.itemIcon} aria-hidden="true">
                  <span
                    className="size-3 rounded-full"
                    style={{ backgroundColor: tagColors[color] }}
                  />
                </span>
                <span className="min-w-0 overflow-hidden text-ellipsis whitespace-nowrap">
                  {name}
                </span>
              </Button>
            </div>
          );
        })}
      </CollapsibleContent>
    </Collapsible>
  );
}
