import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { hasTauriInternals } from "@/shared/platform/tauri";

import { invoke } from "./invoke";
import type { OperationQueueSnapshot } from "./ipc";

/** Kuragine's cloud accounts: the person's own OAuth app per provider. */
export type CloudProviderKind = "gdrive" | "onedrive" | "dropbox";

export interface CloudCredentials {
  clientId: string;
  /** Google desktop clients only. */
  clientSecret?: string;
  /** Microsoft tenant; "common" when empty. */
  tenant?: string;
}

export interface CloudRemote {
  name: string;
  kind: CloudProviderKind;
  label: string;
  email: string;
  displayName: string;
  signedIn: boolean;
  /** The account's local folder under ~/.kura/providers. */
  path: string;
}

export interface CloudQuota {
  usedBytes: number;
  totalBytes: number;
}

export interface CloudConflict {
  remote: string;
  rel: string;
  localPath: string;
  remoteCopyPath: string;
  detectedMs: number;
}

export type CloudConflictChoice = "keep_mine" | "keep_theirs" | "keep_both";

export type CloudEvent =
  | { type: "listingChanged"; remote: string; rel: string; path: string }
  | { type: "copyUpdated"; remote: string; rel: string; path: string }
  | { type: "uploadNeeded"; remote: string; rel: string; path: string }
  | { type: "remotesChanged" }
  | { type: "signInRequired"; remote: string };

export function cloudRemotes(): Promise<CloudRemote[]> {
  return invoke("cloud_remotes");
}

export function cloudAddRemote(
  name: string,
  kind: CloudProviderKind,
  credentials: CloudCredentials,
): Promise<CloudRemote> {
  return invoke("cloud_add_remote", { name, kind, credentials });
}

export function cloudSetCredentials(
  name: string,
  credentials: CloudCredentials,
): Promise<void> {
  return invoke("cloud_set_credentials", { name, credentials });
}

/** Opens the provider's sign-in page and resolves once the browser returns. */
export function cloudSignIn(name: string): Promise<CloudRemote> {
  return invoke("cloud_sign_in", { name });
}

export function cloudCancelSignIn(name: string): Promise<void> {
  return invoke("cloud_cancel_sign_in", { name });
}

export function cloudRemoveRemote(
  name: string,
  deleteFiles: boolean,
): Promise<void> {
  return invoke("cloud_remove_remote", { name, deleteFiles });
}

export function cloudQuota(name: string): Promise<CloudQuota> {
  return invoke("cloud_quota", { name });
}

export function cloudConflicts(): Promise<CloudConflict[]> {
  return invoke("cloud_conflicts");
}

export function cloudResolveConflict(
  path: string,
  choice: CloudConflictChoice,
  operationId?: number,
): Promise<OperationQueueSnapshot> {
  return invoke("cloud_resolve_conflict", {
    path,
    choice,
    operationId: operationId ?? null,
  });
}

export const CLOUD_EVENT = "kura://cloud";

export async function listenCloudEvents(
  handler: (event: CloudEvent) => void,
): Promise<UnlistenFn> {
  if (!hasTauriInternals()) return () => {};
  return listen<CloudEvent>(CLOUD_EVENT, (event) => handler(event.payload));
}
