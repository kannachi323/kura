import type { FileEntry } from "@/native/ipc";

/**
 * Folders macOS shows and opens as single items (Finder's "packages").
 * Opening one launches or opens it; Show Package Contents browses inside.
 */
const packageExtensions = new Set([
  "app",
  "bundle",
  "framework",
  "kext",
  "photoslibrary",
  "pkg",
  "mpkg",
  "plugin",
  "prefpane",
  "qlgenerator",
  "rtfd",
  "saver",
  "scptd",
  "xcodeproj",
  "xcworkspace",
  "playground",
  "key",
  "numbers",
  "pages",
]);

export function isPackageEntry(entry: Pick<FileEntry, "kind" | "extension">) {
  return (
    entry.kind === "folder" &&
    packageExtensions.has(entry.extension.toLowerCase())
  );
}

const kindByExtension: Record<string, string> = {
  app: "Application",
  bundle: "Bundle",
  framework: "Framework",
  pkg: "Installer package",
  mpkg: "Installer package",
  photoslibrary: "Photos Library",
  xcodeproj: "Xcode project",
  xcworkspace: "Xcode workspace",
  rtfd: "RTFD document",
  dmg: "Disk image",
  iso: "Disk image",
  zip: "ZIP archive",
  gz: "Gzip archive",
  tgz: "Gzip tar archive",
  tar: "Tar archive",
  bz2: "Bzip2 archive",
  xz: "XZ archive",
  "7z": "7-Zip archive",
  rar: "RAR archive",
  pdf: "PDF document",
  txt: "Plain text",
  md: "Markdown document",
  markdown: "Markdown document",
  rtf: "Rich text document",
  doc: "Word document",
  docx: "Word document",
  xls: "Excel spreadsheet",
  xlsx: "Excel spreadsheet",
  csv: "CSV document",
  tsv: "TSV document",
  ppt: "PowerPoint presentation",
  pptx: "PowerPoint presentation",
  key: "Keynote presentation",
  numbers: "Numbers spreadsheet",
  pages: "Pages document",
  epub: "EPUB book",
  html: "HTML document",
  htm: "HTML document",
  css: "CSS stylesheet",
  scss: "Sass stylesheet",
  js: "JavaScript source",
  mjs: "JavaScript source",
  cjs: "JavaScript source",
  jsx: "JavaScript source",
  ts: "TypeScript source",
  tsx: "TypeScript source",
  json: "JSON document",
  yaml: "YAML document",
  yml: "YAML document",
  toml: "TOML document",
  xml: "XML document",
  plist: "Property list",
  sql: "SQL source",
  sh: "Shell script",
  zsh: "Shell script",
  bash: "Shell script",
  py: "Python source",
  rb: "Ruby source",
  rs: "Rust source",
  go: "Go source",
  java: "Java source",
  kt: "Kotlin source",
  swift: "Swift source",
  c: "C source",
  h: "C header",
  cpp: "C++ source",
  cc: "C++ source",
  hpp: "C++ header",
  m: "Objective-C source",
  cs: "C# source",
  php: "PHP source",
  lua: "Lua source",
  log: "Log file",
  ics: "Calendar event",
  vcf: "Contact card",
  ttf: "TrueType font",
  otf: "OpenType font",
  woff: "Web font",
  woff2: "Web font",
  png: "PNG image",
  jpg: "JPEG image",
  jpeg: "JPEG image",
  gif: "GIF image",
  webp: "WebP image",
  heic: "HEIC image",
  heif: "HEIF image",
  tif: "TIFF image",
  tiff: "TIFF image",
  bmp: "BMP image",
  svg: "SVG image",
  ico: "Icon image",
  icns: "Apple icon image",
  psd: "Photoshop document",
  raw: "Raw image",
  dng: "DNG image",
  mp3: "MP3 audio",
  m4a: "MPEG-4 audio",
  aac: "AAC audio",
  wav: "WAV audio",
  aiff: "AIFF audio",
  flac: "FLAC audio",
  ogg: "Ogg audio",
  mp4: "MPEG-4 movie",
  m4v: "MPEG-4 movie",
  mov: "QuickTime movie",
  mkv: "Matroska video",
  avi: "AVI movie",
  webm: "WebM video",
  exe: "Windows application",
  msi: "Windows installer",
  dll: "Windows library",
  dylib: "Dynamic library",
  so: "Shared library",
  a: "Static library",
  o: "Object file",
  wasm: "WebAssembly module",
  jar: "Java archive",
  apk: "Android package",
  ipa: "iOS app",
  torrent: "Torrent file",
  webloc: "Web location",
  url: "Internet shortcut",
  lnk: "Shortcut",
  db: "Database",
  sqlite: "SQLite database",
  lock: "Lock file",
  env: "Environment file",
};

const mimeFamily: Record<string, string> = {
  image: "image",
  video: "video",
  audio: "audio",
  text: "text document",
  font: "font",
};

/** Finder-style "Kind": "PNG image", "Folder", "Application". */
export function kindLabel(
  entry: Pick<FileEntry, "kind" | "extension" | "name"> & {
    mimeType?: string | null;
  },
): string {
  const extension = entry.extension.toLowerCase();
  if (entry.kind === "folder")
    return isPackageEntry(entry)
      ? (kindByExtension[extension] ?? "Package")
      : "Folder";
  if (entry.kind === "symlink") return "Alias";
  const known = kindByExtension[extension];
  if (known) return known;
  const family = entry.mimeType ? mimeFamily[entry.mimeType.split("/")[0]] : "";
  if (extension) return `${extension.toUpperCase()} ${family || "file"}`;
  if (entry.name.startsWith(".")) return "Hidden file";
  return family ? capitalise(family) : "Document";
}

function capitalise(text: string): string {
  return text.charAt(0).toUpperCase() + text.slice(1);
}
