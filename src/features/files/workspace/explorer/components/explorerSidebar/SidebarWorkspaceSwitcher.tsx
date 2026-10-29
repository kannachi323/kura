import { sidebarStyles } from "@/features/file-ui";
import { requestOpenSettings } from "@/features/settings";
import { useShortcutTitle } from "@/features/shortcuts";
import { useWorkspaceStore, type Workspace } from "@/features/workspace";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  cn,
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
  IconButton,
  Input,
} from "@/shared/ui";
import {
  Briefcase,
  Check,
  ChevronDown,
  Pencil,
  Plus,
  Settings,
  Trash2,
} from "lucide-react";
import { useRef, useState, type MouseEvent } from "react";

type WorkspaceDialog =
  | { kind: "create" }
  | { kind: "rename"; workspace: Workspace }
  | { kind: "delete"; workspace: Workspace };

/**
 * The top of the Files sidebar: switch between workspaces, each its own set of
 * window tabs, and open Settings.
 */
export function SidebarWorkspaceSwitcher() {
  const workspaces = useWorkspaceStore((state) => state.workspaces);
  const activeId = useWorkspaceStore((state) => state.activeWorkspaceId);
  const active = workspaces.find((workspace) => workspace.id === activeId);
  const [menuOpen, setMenuOpen] = useState(false);
  const [dialog, setDialog] = useState<WorkspaceDialog | null>(null);
  // A dialog picked from the menu opens once the menu has closed; while the
  // menu still traps focus, the dialog's field cannot take it.
  const pendingDialog = useRef<WorkspaceDialog | null>(null);
  const openFromMenu = (next: WorkspaceDialog) => {
    pendingDialog.current = next;
    setMenuOpen(false);
  };
  const settingsTitle = useShortcutTitle("Settings", "app.open_settings");
  const rowAction =
    (next: WorkspaceDialog) => (event: MouseEvent<HTMLButtonElement>) => {
      // Keep the row from also switching to the workspace.
      event.preventDefault();
      event.stopPropagation();
      openFromMenu(next);
    };

  return (
    <section className={sidebarStyles.section}>
      <div className="flex min-w-0 items-center gap-1.5">
        <DropdownMenu open={menuOpen} onOpenChange={setMenuOpen}>
          <DropdownMenuTrigger asChild>
            <Button
              type="button"
              variant="ghost"
              className={cn(
                sidebarStyles.workspaceSelect,
                "w-auto min-w-0 flex-1",
              )}
            >
              <Briefcase aria-hidden="true" />
              <span className={sidebarStyles.workspaceSelectLabel}>
                {active?.title ?? "Workspace"}
              </span>
              <ChevronDown aria-hidden="true" className="!size-4" />
            </Button>
          </DropdownMenuTrigger>
          <DropdownMenuContent
            align="start"
            className="w-60"
            onCloseAutoFocus={(event) => {
              if (!pendingDialog.current) return;
              event.preventDefault();
              setDialog(pendingDialog.current);
              pendingDialog.current = null;
            }}
          >
            {workspaces.map((workspace) => (
              <DropdownMenuItem
                key={workspace.id}
                className="group/workspace gap-2 pr-1"
                role="menuitemradio"
                aria-checked={workspace.id === activeId}
                onSelect={() =>
                  useWorkspaceStore.getState().switchWorkspace(workspace.id)
                }
              >
                <span className="grid w-[15px] flex-none place-items-center">
                  {workspace.id === activeId ? <Check size={15} /> : null}
                </span>
                <span className="min-w-0 flex-1 truncate">
                  {workspace.title}
                </span>
                <span className="flex flex-none items-center gap-px opacity-0 group-hover/workspace:opacity-100 group-focus-within/workspace:opacity-100 group-data-[highlighted]/workspace:opacity-100">
                  <IconButton
                    size="xs"
                    tooltip={false}
                    label={`Rename ${workspace.title}`}
                    onPointerDown={(event) => event.stopPropagation()}
                    onClick={rowAction({ kind: "rename", workspace })}
                  >
                    <Pencil size={14} />
                  </IconButton>
                  {workspaces.length > 1 ? (
                    <IconButton
                      size="xs"
                      tooltip={false}
                      label={`Delete ${workspace.title}`}
                      onPointerDown={(event) => event.stopPropagation()}
                      onClick={rowAction({ kind: "delete", workspace })}
                    >
                      <Trash2 size={14} />
                    </IconButton>
                  ) : null}
                </span>
              </DropdownMenuItem>
            ))}
            <DropdownMenuSeparator />
            <DropdownMenuItem
              className="gap-2"
              onSelect={() => openFromMenu({ kind: "create" })}
            >
              <Plus size={15} />
              <span>New workspace</span>
            </DropdownMenuItem>
          </DropdownMenuContent>
        </DropdownMenu>
        <IconButton
          label="Settings"
          tooltip={settingsTitle}
          className="size-10 flex-none"
          onClick={requestOpenSettings}
        >
          <Settings size={18} />
        </IconButton>
      </div>
      {dialog?.kind === "delete" ? (
        <DeleteWorkspaceDialog
          workspace={dialog.workspace}
          onClose={() => setDialog(null)}
        />
      ) : dialog ? (
        <WorkspaceNameDialog
          workspace={dialog.kind === "rename" ? dialog.workspace : null}
          onClose={() => setDialog(null)}
        />
      ) : null}
    </section>
  );
}

function WorkspaceNameDialog(props: {
  /** Renames this workspace, or creates one when null. */
  workspace: Workspace | null;
  onClose: () => void;
}) {
  const [name, setName] = useState(props.workspace?.title ?? "");
  const save = () => {
    const store = useWorkspaceStore.getState();
    if (props.workspace) store.renameWorkspace(props.workspace.id, name);
    else store.createWorkspace(name);
    props.onClose();
  };
  return (
    <Dialog
      open
      onOpenChange={(open) => {
        if (!open) props.onClose();
      }}
    >
      <DialogContent className="sm:max-w-sm">
        <form
          className="contents"
          onSubmit={(event) => {
            event.preventDefault();
            if (props.workspace && !name.trim()) return;
            save();
          }}
        >
          <DialogHeader>
            <DialogTitle>
              {props.workspace ? "Rename Workspace" : "New Workspace"}
            </DialogTitle>
          </DialogHeader>
          <Input
            autoFocus
            aria-label="Workspace name"
            placeholder={props.workspace ? undefined : "Untitled workspace"}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
          <DialogFooter>
            <Button variant="outline" type="button" onClick={props.onClose}>
              Cancel
            </Button>
            <Button
              type="submit"
              disabled={Boolean(props.workspace) && !name.trim()}
            >
              {props.workspace ? "Rename" : "Create"}
            </Button>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}

function DeleteWorkspaceDialog(props: {
  workspace: Workspace;
  onClose: () => void;
}) {
  return (
    <AlertDialog
      open
      onOpenChange={(open) => {
        if (!open) props.onClose();
      }}
    >
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Delete Workspace</AlertDialogTitle>
          <AlertDialogDescription>
            Delete{" "}
            <strong className="font-semibold text-cream">
              {props.workspace.title}
            </strong>{" "}
            and close its tabs? Your files are not affected.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            className="bg-charcoal-active text-cream-bright"
            onClick={() =>
              useWorkspaceStore.getState().deleteWorkspace(props.workspace.id)
            }
          >
            Delete
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
