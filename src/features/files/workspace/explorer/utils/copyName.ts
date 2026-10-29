/**
 * Finder's name for a duplicate: "report copy.txt", then "report copy 2.txt".
 * Folders keep dots in their names, so they get no extension split.
 */
export function copyName(
  name: string,
  isFolder: boolean,
  taken: ReadonlySet<string>,
): string {
  const dot = isFolder ? -1 : name.lastIndexOf(".");
  const hasExtension = dot > 0 && dot < name.length - 1;
  const stem = hasExtension ? name.slice(0, dot) : name;
  const extension = hasExtension ? name.slice(dot) : "";
  for (let index = 1; ; index += 1) {
    const candidate = `${stem} copy${index === 1 ? "" : ` ${index}`}${extension}`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
}

/** "Untitled Folder", then "Untitled Folder 2", skipping names in use. */
export function uniqueName(base: string, taken: ReadonlySet<string>): string {
  if (!taken.has(base.toLowerCase())) return base;
  for (let index = 2; ; index += 1) {
    const candidate = `${base} ${index}`;
    if (!taken.has(candidate.toLowerCase())) return candidate;
  }
}
