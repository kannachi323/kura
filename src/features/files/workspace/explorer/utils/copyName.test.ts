import { describe, expect, it } from "vitest";
import { copyName, uniqueName } from "./copyName";

describe("copyName", () => {
  it("adds copy before the extension, then numbers further copies", () => {
    expect(copyName("report.txt", false, new Set())).toBe("report copy.txt");
    expect(copyName("report.txt", false, new Set(["report copy.txt"]))).toBe(
      "report copy 2.txt",
    );
  });

  it("keeps folder names whole and handles dotfiles", () => {
    expect(copyName("v1.2", true, new Set())).toBe("v1.2 copy");
    expect(copyName(".env", false, new Set())).toBe(".env copy");
  });

  it("compares names case-insensitively, like APFS", () => {
    expect(uniqueName("Untitled Folder", new Set(["untitled folder"]))).toBe(
      "Untitled Folder 2",
    );
  });
});
