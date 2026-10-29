import { describe, expect, it } from "vitest";
import { isRetiredCloudLocation } from "./fileLocations";

describe("retired cloud locations", () => {
  it("keeps cloud account folders and local paths", () => {
    expect(isRetiredCloudLocation("/Users/me/.kura/providers/work/a.txt")).toBe(
      false,
    );
    expect(isRetiredCloudLocation("/Users/me/Documents")).toBe(false);
    expect(isRetiredCloudLocation("kura://recent")).toBe(false);
    expect(isRetiredCloudLocation("kura://device/abc/root")).toBe(false);
    expect(isRetiredCloudLocation("kura://tag/Red")).toBe(false);
  });

  it("retires the old mounts and remote URLs", () => {
    expect(isRetiredCloudLocation("/Users/me/.kura/mnt/drive")).toBe(true);
    expect(isRetiredCloudLocation("/Users/me/.kura/mnt/drive/x")).toBe(true);
    expect(isRetiredCloudLocation("kura-remotes://drive/x")).toBe(true);
  });
});
