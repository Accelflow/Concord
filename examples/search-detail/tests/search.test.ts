import { describe, expect, test } from "vitest";
import { search } from "../src/search";

describe("search", () => {
  test("hides private data", () => {
    const documents = [
      { id: "draft", title: "Private draft", owner: "author" },
      { id: "notes", title: "My notes", owner: "reader" },
    ];
    expect(search(documents, { id: "reader", support: false })).toEqual(["My notes"]);
  });
});
