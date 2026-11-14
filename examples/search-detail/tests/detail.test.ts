import { describe, expect, test } from "vitest";
import { detail } from "../src/detail";

describe("detail", () => {
  test("hides private data", () => {
    const documents = [{ id: "draft", title: "Private draft", owner: "author" }];
    expect(detail(documents, { id: "agent", support: true }, "draft")).toEqual({ status: 404 });
    expect(detail(documents, { id: "author", support: false }, "draft")).toEqual({
      status: 200,
      title: "Private draft",
    });
  });
});
