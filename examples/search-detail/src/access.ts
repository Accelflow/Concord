export type Document = { id: string; title: string; owner: string };
export type Viewer = { id: string; support: boolean };

export function canRead(viewer: Viewer, document: Document): boolean {
  return viewer.id === document.owner;
}
