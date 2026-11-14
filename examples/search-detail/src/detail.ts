import { canRead, type Document, type Viewer } from "./access";

export function detail(documents: Document[], viewer: Viewer, id: string) {
  const document = documents.find((candidate) => candidate.id === id);
  if (!document || !canRead(viewer, document)) {
    return { status: 404 };
  }
  return { status: 200, title: document.title };
}
