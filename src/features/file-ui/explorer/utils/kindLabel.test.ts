import { describe, expect, it } from "vitest";
import { isPackageEntry, kindLabel } from "./kindLabel";

const entry = (name: string, kind: "file" | "folder" = "file") => ({
  name,
  kind,
  extension: name.includes(".") ? name.split(".").pop()! : "",
  mimeType: null,
});

describe("kindLabel", () => {
  it("names common file types the way Finder does", () => {
    expect(kindLabel(entry("photo.PNG"))).toBe("PNG image");
    expect(kindLabel(entry("main.rs"))).toBe("Rust source");
    expect(kindLabel(entry("Docs", "folder"))).toBe("Folder");
  });

  it("treats app bundles as applications, not folders", () => {
    expect(isPackageEntry(entry("Safari.app", "folder"))).toBe(true);
    expect(kindLabel(entry("Safari.app", "folder"))).toBe("Application");
    expect(isPackageEntry(entry("v1.2", "folder"))).toBe(false);
  });

  it("falls back to the extension, then the MIME family", () => {
    expect(kindLabel(entry("data.xyz"))).toBe("XYZ file");
    expect(
      kindLabel({ ...entry("clip.qqq"), mimeType: "video/x-custom" }),
    ).toBe("QQQ video");
    expect(kindLabel(entry("Makefile"))).toBe("Document");
  });
});
