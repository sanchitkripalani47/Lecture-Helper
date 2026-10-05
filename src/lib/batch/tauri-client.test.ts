import { describe, expect, it } from "vitest";
import { defaultOutputRoot, joinPath } from "./tauri-client";

describe("defaultOutputRoot", () => {
  it("creates a sibling folder for a POSIX path", () => {
    expect(defaultOutputRoot("/Users/you/My Videos")).toBe("/Users/you/My Videos - Processed");
  });

  it("ignores a trailing separator", () => {
    expect(defaultOutputRoot("/Users/you/My Videos/")).toBe("/Users/you/My Videos - Processed");
  });

  it("keeps Windows separators", () => {
    expect(defaultOutputRoot("C:\\Users\\you\\My Videos")).toBe(
      "C:\\Users\\you\\My Videos - Processed",
    );
  });

  it("handles a top-level folder without a parent", () => {
    expect(defaultOutputRoot("My Videos")).toBe("My Videos - Processed");
  });
});

describe("joinPath", () => {
  it("joins with the existing separator style", () => {
    expect(joinPath("/a/b", "c")).toBe("/a/b/c");
    expect(joinPath("C:\\a\\b", "c")).toBe("C:\\a\\b\\c");
  });

  it("does not double up separators", () => {
    expect(joinPath("/a/b/", "c")).toBe("/a/b/c");
  });

  it("returns the segment when the base is empty", () => {
    expect(joinPath("", "c")).toBe("c");
  });
});
