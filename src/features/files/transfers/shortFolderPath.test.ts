import { describe, expect, it } from "vitest";
import { shortFolderPath } from "./transferModel";

describe("shortFolderPath", () => {
  it("shows the folder, with Home as ~", () => {
    expect(shortFolderPath("/Users/me/Documents/a.txt", "/Users/me")).toBe(
      "~/Documents",
    );
    expect(shortFolderPath("/Volumes/Backup/a.txt", "/Users/me")).toBe(
      "/Volumes/Backup",
    );
  });

  it("keeps the last two folders of deep paths", () => {
    expect(
      shortFolderPath("/Users/me/projects/kura/docs/notes/a.md", "/Users/me"),
    ).toBe("…/docs/notes");
  });
});
