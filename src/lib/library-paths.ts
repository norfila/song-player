/** Folder boundaries matter: C:/Music must not cover C:/Musical. Folders the
 * user removed from inside a library folder are not covered. */
export function isLibraryFolderCovered(folder: string, libraryFolders: readonly string[], excludedFolders: readonly string[] = []): boolean {
  const windows = /^[a-z]:[\\/]/i.test(folder) || /^[\\/]{2}/.test(folder);
  const normalize = (value: string) => {
    const normalized = value.replace(/\\/g, '/').replace(/\/+$/, '') || '/';
    return windows ? normalized.toLocaleLowerCase('en-US') : normalized;
  };
  const target = normalize(folder);
  const inside = (folders: readonly string[]) => folders.some(folder => { const root = normalize(folder); return root === target || (root === '/' ? target.startsWith('/') : target.startsWith(`${root}/`)); });
  return inside(libraryFolders) && !inside(excludedFolders);
}
