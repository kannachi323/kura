import { describe, expect, it } from "vitest";
import { ancestorPaths } from "./ColumnView";

describe("ancestorPaths", () => {
  it("starts at Home for folders inside it", () => {
    expect(ancestorPaths("/Users/me/projects/kura", "/Users/me")).toEqual([
      "/Users/me",
      "/Users/me/projects",
    ]);
    expect(ancestorPaths("/Users/me", "/Users/me")).toEqual([]);
  });

  it("starts at the root elsewhere, and skips virtual locations", () => {
    expect(ancestorPaths("/Applications/Utilities", "/Users/me")).toEqual([
      "/",
      "/Applications",
    ]);
    expect(ancestorPaths("/", "/Users/me")).toEqual([]);
    expect(ancestorPaths("kura://trash", "/Users/me")).toEqual([]);
  });
});
