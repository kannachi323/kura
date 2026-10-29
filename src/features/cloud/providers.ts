import type { CloudProviderKind } from "@/native/cloud";
import dropboxLogo from "./assets/dropbox.svg";
import googleDriveLogo from "./assets/google-drive.svg";
import oneDriveLogo from "./assets/onedrive.svg";

/** Dropbox matches redirect URIs exactly, so Kura signs in on a fixed port. */
export const DROPBOX_REDIRECT_URI = "http://localhost:53682/";

export interface CloudProviderGuide {
  kind: CloudProviderKind;
  label: string;
  logo: string;
  /** Where the person registers their own OAuth app. */
  consoleUrl: string;
  consoleLabel: string;
  /** A redirect URI the app registration must list, if any. */
  redirectUri: string | null;
  clientIdLabel: string;
  /** Google's desktop clients come with a secret Kura must send. */
  needsSecret: boolean;
  /** Microsoft accounts can be limited to one tenant. */
  hasTenant: boolean;
  steps: string[];
}

export const cloudProviderGuides: readonly CloudProviderGuide[] = [
  {
    kind: "gdrive",
    label: "Google Drive",
    logo: googleDriveLogo,
    consoleUrl: "https://console.cloud.google.com/apis/credentials",
    consoleLabel: "Open Google Cloud Console",
    redirectUri: null,
    clientIdLabel: "Client ID",
    needsSecret: true,
    hasTenant: false,
    steps: [
      "Create a project in Google Cloud Console and enable the Google Drive API.",
      "Set up the OAuth consent screen for External users and add yourself as a test user.",
      "Publish the app (In production). Otherwise Google signs Kura out every 7 days.",
      "Create an OAuth client ID of type Desktop app.",
      "Paste its client ID and client secret below.",
    ],
  },
  {
    kind: "onedrive",
    label: "OneDrive",
    logo: oneDriveLogo,
    consoleUrl:
      "https://entra.microsoft.com/#view/Microsoft_AAD_RegisteredApps/ApplicationsListBlade",
    consoleLabel: "Open Microsoft Entra",
    redirectUri: "http://localhost",
    clientIdLabel: "Application (client) ID",
    needsSecret: false,
    hasTenant: true,
    steps: [
      "Register a new app in Microsoft Entra. For account types, choose any organizational directory and personal Microsoft accounts.",
      "Add the platform Mobile and desktop applications with the redirect URI below.",
      "Under Authentication, turn on Allow public client flows.",
      "Paste the application (client) ID below. No secret is needed.",
    ],
  },
  {
    kind: "dropbox",
    label: "Dropbox",
    logo: dropboxLogo,
    consoleUrl: "https://www.dropbox.com/developers/apps",
    consoleLabel: "Open the Dropbox App Console",
    redirectUri: DROPBOX_REDIRECT_URI,
    clientIdLabel: "App key",
    needsSecret: false,
    hasTenant: false,
    steps: [
      "Create an app with Scoped access and Full Dropbox access.",
      "Under Permissions, allow files.metadata.read and .write, files.content.read and .write, and account_info.read, then submit.",
      "Under OAuth 2, add the redirect URI below.",
      "Paste the app key below. Kura uses PKCE, so the app secret isn't needed.",
    ],
  },
];

export function cloudProviderGuide(
  kind: CloudProviderKind,
): CloudProviderGuide {
  return (
    cloudProviderGuides.find((guide) => guide.kind === kind) ??
    cloudProviderGuides[0]
  );
}
