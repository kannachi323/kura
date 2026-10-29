import {
  SidebarSectionHeader,
  pathIsInside,
  sidebarStyles,
} from "@/features/file-ui";
import { openSettingsAt } from "@/features/settings/openSettings";
import {
  Button,
  cn,
  Collapsible,
  CollapsibleContent,
  IconButton,
} from "@/shared/ui";
import { Plus } from "lucide-react";
import { useEffect, useState, type ComponentType, type ReactNode } from "react";
import { ProviderLogo } from "./ProviderLogo";
import { useCloudStore } from "./useCloudStore";

const openStorageKey = "kura.sidebar.cloud.open";

function readOpen(): boolean {
  try {
    return window.localStorage.getItem(openStorageKey) !== "false";
  } catch {
    return true;
  }
}

type DropTarget = ComponentType<{
  id: string;
  path: string;
  springLoad?: boolean;
  onSpringLoad?: () => void;
  children: ReactNode;
}>;

/** Cloud accounts in the sidebar. Each opens its folder under ~/.kura/providers. */
export function CloudSidebarSection(props: {
  activePath: string;
  onNavigate: (path: string) => void;
  DropTarget?: DropTarget;
}) {
  const [open, setOpen] = useState(readOpen);
  const remotes = useCloudStore((state) => state.remotes);
  const loaded = useCloudStore((state) => state.loaded);
  useEffect(() => {
    if (!loaded) void useCloudStore.getState().load();
  }, [loaded]);

  const toggle = () => {
    const next = !open;
    setOpen(next);
    try {
      window.localStorage.setItem(openStorageKey, String(next));
    } catch {
      // Private storage modes keep the default.
    }
  };
  const addAccount = () => openSettingsAt("cloud");
  const Target = props.DropTarget;

  return (
    <Collapsible className={sidebarStyles.section} open={open}>
      <SidebarSectionHeader
        title="Cloud"
        collapsed={!open}
        onToggle={toggle}
        actions={
          <IconButton
            size="xs"
            tooltip={false}
            label="Add cloud account"
            className={sidebarStyles.sectionActionButton}
            onClick={(event) => {
              event.stopPropagation();
              addAccount();
            }}
          >
            <Plus size={15} />
          </IconButton>
        }
      />
      <CollapsibleContent className={sidebarStyles.list}>
        {loaded && remotes.length === 0 ? (
          <p className={sidebarStyles.muted}>
            Connect Google Drive, OneDrive or Dropbox with your own app.{" "}
            <button
              type="button"
              className="text-cream-bright underline-offset-4 hover:underline"
              onClick={addAccount}
            >
              Add an account
            </button>
          </p>
        ) : null}
        {remotes.map((remote) => {
          const row = (
            <Button
              type="button"
              variant="ghost"
              className={cn(
                sidebarStyles.itemButton,
                pathIsInside(props.activePath, remote.path) &&
                  sidebarStyles.itemSelected,
              )}
              onClick={() =>
                remote.signedIn ? props.onNavigate(remote.path) : addAccount()
              }
              title={
                remote.signedIn
                  ? remote.email || remote.label
                  : `Sign in to ${remote.name} again in Settings`
              }
            >
              <span className={sidebarStyles.itemIcon} aria-hidden="true">
                <ProviderLogo kind={remote.kind} size={16} />
              </span>
              <span className="min-w-0 flex-1 truncate">{remote.name}</span>
              {remote.signedIn ? null : (
                <small className="ml-auto shrink-0 text-[11px] text-cream-muted">
                  Sign in
                </small>
              )}
            </Button>
          );
          return (
            <div className={sidebarStyles.pinnedRow} key={remote.name}>
              {Target && remote.signedIn ? (
                <Target
                  id={`sidebar:cloud:${remote.name}`}
                  path={remote.path}
                  springLoad
                  onSpringLoad={() => props.onNavigate(remote.path)}
                >
                  {row}
                </Target>
              ) : (
                row
              )}
            </div>
          );
        })}
      </CollapsibleContent>
    </Collapsible>
  );
}
