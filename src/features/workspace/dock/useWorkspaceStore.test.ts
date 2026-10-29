import { beforeEach, describe, expect, it } from "vitest";
import { allLayoutViews } from "./layoutTabs";
import { useWorkspaceStore } from "./useWorkspaceStore";

const store = useWorkspaceStore.getState;

function openFolder(path: string) {
  return store().openSurface({ surfaceId: "files", state: { path } });
}

describe("workspaces", () => {
  beforeEach(() => {
    localStorage.clear();
    useWorkspaceStore.setState(useWorkspaceStore.getInitialState(), true);
  });

  it("starts with one workspace holding the window's tabs", () => {
    expect(store().workspaces.map((w) => w.title)).toEqual(["Workspace 1"]);
    expect(store().activeWorkspaceId).toBe(store().workspaces[0].id);
  });

  it("swaps every tab when switching and restores them on return", () => {
    const first = store().activeWorkspaceId;
    openFolder("/Users/test/Documents");
    const firstTabs = store().layout.tabs.map((tab) => tab.id);

    const second = store().createWorkspace();
    expect(second.title).toBe("Workspace 2");
    expect(store().activeWorkspaceId).toBe(second.id);
    expect(store().layout.tabs).toHaveLength(1);
    openFolder("/Users/test/Music");

    expect(store().switchWorkspace(first)).toBe(true);
    expect(store().layout.tabs.map((tab) => tab.id)).toEqual(firstTabs);

    store().switchWorkspace(second.id);
    expect(
      allLayoutViews(store().layout).map((view) => view.state.path),
    ).toContain("/Users/test/Music");
  });

  it("renames, and deletes all but the last workspace", () => {
    const first = store().activeWorkspaceId;
    const second = store().createWorkspace("Photos");
    store().renameWorkspace(second.id, "  Archive  ");
    store().renameWorkspace(second.id, "   ");
    expect(store().workspaces[1].title).toBe("Archive");

    expect(store().deleteWorkspace(second.id)).toBe(true);
    expect(store().activeWorkspaceId).toBe(first);
    expect(store().deleteWorkspace(first)).toBe(false);
    expect(store().workspaces).toHaveLength(1);
  });

  it("names new workspaces after the first free number", () => {
    const second = store().createWorkspace();
    store().createWorkspace();
    store().deleteWorkspace(second.id);
    expect(store().createWorkspace().title).toBe("Workspace 2");
  });

  it("turns a layout saved before workspaces into the first workspace", async () => {
    const { layout } = useWorkspaceStore.getInitialState();
    const retired = {
      ...layout.tabs[0].root,
      id: "pane:old",
      views: [
        {
          ...allLayoutViews(layout)[0],
          id: "tab:old",
          surfaceId: "extensions",
        },
      ],
    };
    localStorage.setItem(
      "kura:workspace-layout",
      JSON.stringify({
        version: 1,
        state: {
          layout: {
            ...layout,
            tabs: [
              ...layout.tabs,
              { id: "layout:old", root: retired, focusedPaneId: "pane:old" },
            ],
          },
        },
      }),
    );
    await useWorkspaceStore.persist.rehydrate();

    expect(store().workspaces.map((w) => w.title)).toEqual(["Workspace 1"]);
    // Tabs that only showed a retired page are dropped.
    expect(store().layout.tabs.map((tab) => tab.id)).toEqual([
      layout.tabs[0].id,
    ]);
  });
});
