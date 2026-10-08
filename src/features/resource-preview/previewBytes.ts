/** Reads preview bytes from an asset URL supplied by the native file-preview command. */
export async function fetchPreviewBytes(url: string): Promise<ArrayBuffer> {
  const response = await fetch(url);
  if (!response.ok)
    throw new Error(`The file reader returned ${response.status}.`);
  return response.arrayBuffer();
}
