/** Converts a UTF-8 byte offset into a UTF-16 string index. */
export function byteToIndex(source: string, byteOffset: number): number {
  let bytes = 0;
  let index = 0;
  while (index < source.length && bytes < byteOffset) {
    const code = source.codePointAt(index)!;
    bytes += code < 0x80 ? 1 : code < 0x800 ? 2 : code < 0x10000 ? 3 : 4;
    index += code >= 0x10000 ? 2 : 1;
  }
  return index;
}

/** Strips extensions that MiniJinja ignores for auto escaping. */
export function stripTemplateExtension(name: string): string {
  for (const ext of [".j2", ".jinja2", ".jinja"]) {
    if (name.endsWith(ext)) {
      return name.slice(0, -ext.length);
    }
  }
  return name;
}

/** Returns the extension of a file name (without the dot). */
export function extension(name: string): string {
  const idx = name.lastIndexOf(".");
  return idx <= 0 ? "" : name.slice(idx + 1).toLowerCase();
}

/** Describes the auto escaping MiniJinja applies based on a file name. */
export function autoEscapeFor(name: string): string {
  const ext = extension(stripTemplateExtension(name));
  if (["html", "htm", "xml"].includes(ext)) {
    return "HTML";
  } else if (["json", "json5", "js", "yaml", "yml"].includes(ext)) {
    return "JSON";
  }
  return "none";
}
