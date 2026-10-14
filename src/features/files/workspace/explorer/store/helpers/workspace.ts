export function isExplorerInternalTabPath(path: string): boolean {
  return (
    path.startsWith("kura-transfers://") || path.startsWith("kura-plugin://")
  );
}
