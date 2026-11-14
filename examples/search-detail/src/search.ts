import { canRead, type Document, type Viewer } from "./access";

export function search(documents: Document[], viewer: Viewer): string[] {
  return documents.filter((document) => canRead(viewer, document)).map((document) => document.title);
}
