import {
  DesktopSettingsRow as Row,
  DesktopSettingsSection as Section,
  SettingsNote,
} from "@/features/settings/components/DesktopSettingsUI";
import {
  cloudAddRemote,
  cloudCancelSignIn,
  cloudConflicts,
  cloudQuota,
  cloudRemoveRemote,
  cloudResolveConflict,
  cloudSignIn,
  type CloudConflict,
  type CloudConflictChoice,
  type CloudProviderKind,
  type CloudQuota,
  type CloudRemote,
} from "@/native/cloud";
import { formatBytes } from "@/shared/lib/fileFormat";
import { errorText } from "@/shared/lib/format";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  Checkbox,
  Input,
  SegmentedControl,
} from "@/shared/ui";
import { openUrl } from "@tauri-apps/plugin-opener";
import { Copy, ExternalLink } from "lucide-react";
import { useEffect, useState } from "react";
import { cloudProviderGuide, cloudProviderGuides } from "./providers";
import { ProviderLogo } from "./ProviderLogo";
import { useCloudStore } from "./useCloudStore";

/** Settings → Cloud: accounts connected with the person's own OAuth apps. */
export function CloudAccountsSettings() {
  const remotes = useCloudStore((state) => state.remotes);
  const loaded = useCloudStore((state) => state.loaded);
  const loadError = useCloudStore((state) => state.error);
  const [conflicts, setConflicts] = useState<CloudConflict[]>([]);
  const reloadConflicts = () =>
    void cloudConflicts()
      .then(setConflicts)
      .catch(() => setConflicts([]));
  useEffect(() => {
    void useCloudStore.getState().load();
    reloadConflicts();
  }, []);

  return (
    <>
      <Section
        title="Accounts"
        description="Kura signs in to each cloud with an app you register yourself, so no Kura server ever sees your files or sign-ins. Downloaded files live in ~/.kura/providers, and edits you save there upload automatically."
      >
        {loadError ? <SettingsNote>{loadError}</SettingsNote> : null}
        {loaded && remotes.length === 0 ? (
          <SettingsNote>No cloud accounts yet. Add one below.</SettingsNote>
        ) : null}
        {remotes.map((remote) => (
          <RemoteRow key={remote.name} remote={remote} />
        ))}
      </Section>
      <AddAccount takenNames={remotes.map((remote) => remote.name)} />
      {conflicts.length > 0 ? (
        <Section
          title="Unresolved conflicts"
          description="These files changed here and in the cloud. Uploads of them wait until you choose a version."
        >
          {conflicts.map((conflict) => (
            <ConflictRow
              key={`${conflict.remote}:${conflict.rel}`}
              conflict={conflict}
              onResolved={reloadConflicts}
            />
          ))}
        </Section>
      ) : null}
    </>
  );
}

function RemoteRow({ remote }: { remote: CloudRemote }) {
  const [quota, setQuota] = useState<CloudQuota | null>(null);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [removing, setRemoving] = useState(false);
  useEffect(() => {
    if (!remote.signedIn) return;
    let active = true;
    void cloudQuota(remote.name)
      .then((value) => active && setQuota(value))
      .catch(() => undefined);
    return () => {
      active = false;
    };
  }, [remote.name, remote.signedIn]);

  const signIn = async () => {
    setWorking(true);
    setError(null);
    try {
      await cloudSignIn(remote.name);
      await useCloudStore.getState().load();
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setWorking(false);
    }
  };
  const details = [
    remote.label,
    remote.signedIn ? remote.email : "Not signed in",
    quota && quota.totalBytes > 0
      ? `${formatBytes(quota.usedBytes)} of ${formatBytes(quota.totalBytes)} used`
      : null,
  ]
    .filter(Boolean)
    .join(" · ");

  return (
    <>
      <Row label={remote.name} description={error ?? details}>
        <ProviderLogo kind={remote.kind} size={18} />
        {working ? (
          <Button
            variant="outline"
            size="sm"
            onClick={() => void cloudCancelSignIn(remote.name)}
          >
            Cancel sign-in
          </Button>
        ) : (
          <Button
            variant={remote.signedIn ? "outline" : "primary"}
            size="sm"
            onClick={() => void signIn()}
          >
            {remote.signedIn ? "Sign in again" : "Sign in"}
          </Button>
        )}
        <Button
          variant="outline"
          size="sm"
          disabled={working}
          onClick={() => setRemoving(true)}
        >
          Remove
        </Button>
      </Row>
      {removing ? (
        <RemoveDialog remote={remote} onClose={() => setRemoving(false)} />
      ) : null}
    </>
  );
}

function RemoveDialog(props: { remote: CloudRemote; onClose: () => void }) {
  const [deleteFiles, setDeleteFiles] = useState(false);
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const remove = async () => {
    setWorking(true);
    setError(null);
    try {
      await cloudRemoveRemote(props.remote.name, deleteFiles);
      await useCloudStore.getState().load();
      props.onClose();
    } catch (cause) {
      setError(errorText(cause));
      setWorking(false);
    }
  };
  return (
    <AlertDialog open>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Remove “{props.remote.name}”?</AlertDialogTitle>
          <AlertDialogDescription>
            Kura signs out and forgets this account. Nothing in the cloud
            changes.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <label className="flex items-center gap-2 text-sm">
          <Checkbox
            checked={deleteFiles}
            onCheckedChange={(checked) => setDeleteFiles(checked === true)}
            disabled={working}
          />
          Also delete its downloaded files from this computer
        </label>
        {error ? (
          <p role="alert" className="text-sm text-cream">
            {error}
          </p>
        ) : null}
        <AlertDialogFooter>
          <Button variant="ghost" disabled={working} onClick={props.onClose}>
            Cancel
          </Button>
          <Button
            variant="destructive"
            disabled={working}
            onClick={() => void remove()}
          >
            Remove
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}

function uniqueName(base: string, taken: string[]): string {
  const lower = new Set(taken.map((name) => name.toLowerCase()));
  if (!lower.has(base.toLowerCase())) return base;
  for (let n = 2; ; n += 1) {
    const candidate = `${base} ${n}`;
    if (!lower.has(candidate.toLowerCase())) return candidate;
  }
}

function AddAccount(props: { takenNames: string[] }) {
  const [kind, setKind] = useState<CloudProviderKind>("gdrive");
  const guide = cloudProviderGuide(kind);
  const [name, setName] = useState("");
  const [clientId, setClientId] = useState("");
  const [clientSecret, setClientSecret] = useState("");
  const [tenant, setTenant] = useState("");
  const [phase, setPhase] = useState<"idle" | "adding" | "waiting">("idle");
  const [pendingName, setPendingName] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [copied, setCopied] = useState(false);
  const accountName = name.trim() || uniqueName(guide.label, props.takenNames);
  const ready =
    clientId.trim() !== "" &&
    (!guide.needsSecret || clientSecret.trim() !== "");

  const reset = () => {
    setName("");
    setClientId("");
    setClientSecret("");
    setTenant("");
  };
  const add = async () => {
    setPhase("adding");
    setError(null);
    try {
      const remote = await cloudAddRemote(accountName, kind, {
        clientId: clientId.trim(),
        clientSecret: guide.needsSecret ? clientSecret.trim() : undefined,
        tenant: guide.hasTenant && tenant.trim() ? tenant.trim() : undefined,
      });
      await useCloudStore.getState().load();
      reset();
      setPendingName(remote.name);
      setPhase("waiting");
      await cloudSignIn(remote.name);
      await useCloudStore.getState().load();
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setPhase("idle");
      setPendingName("");
    }
  };
  const copyRedirect = () => {
    if (!guide.redirectUri) return;
    void navigator.clipboard
      .writeText(guide.redirectUri)
      .then(() => {
        setCopied(true);
        window.setTimeout(() => setCopied(false), 1500);
      })
      .catch(() => undefined);
  };

  return (
    <Section
      title="Add an account"
      description="Register an app with the provider (one time), then paste its credentials here."
    >
      <Row label="Provider">
        <SegmentedControl
          label="Cloud provider"
          value={kind}
          onChange={(next) => {
            setKind(next);
            setError(null);
          }}
          options={cloudProviderGuides.map((option) => ({
            value: option.kind,
            label: option.label,
            icon: <ProviderLogo kind={option.kind} size={14} />,
          }))}
        />
      </Row>
      <div className="border-b border-charcoal-border px-5 py-4 text-[13px] leading-5 text-cream-muted">
        <ol className="m-0 grid list-decimal gap-1.5 pl-5">
          {guide.steps.map((step) => (
            <li key={step}>{step}</li>
          ))}
        </ol>
        <div className="mt-3 flex flex-wrap items-center gap-2">
          <Button
            variant="outline"
            size="sm"
            onClick={() => void openUrl(guide.consoleUrl)}
          >
            <ExternalLink />
            {guide.consoleLabel}
          </Button>
          {guide.redirectUri ? (
            <Button variant="outline" size="sm" onClick={copyRedirect}>
              <Copy />
              {copied ? "Copied" : `Copy ${guide.redirectUri}`}
            </Button>
          ) : null}
        </div>
      </div>
      <Row
        label="Name"
        description="Shown in the sidebar, and the name of its folder in ~/.kura/providers."
      >
        <Input
          value={name}
          placeholder={uniqueName(guide.label, props.takenNames)}
          onChange={(event) => setName(event.target.value)}
        />
      </Row>
      <Row label={guide.clientIdLabel}>
        <Input
          value={clientId}
          spellCheck={false}
          autoComplete="off"
          onChange={(event) => setClientId(event.target.value)}
        />
      </Row>
      {guide.needsSecret ? (
        <Row
          label="Client secret"
          description="Kept in your system keychain with the sign-in."
        >
          <Input
            type="password"
            value={clientSecret}
            autoComplete="off"
            onChange={(event) => setClientSecret(event.target.value)}
          />
        </Row>
      ) : null}
      {guide.hasTenant ? (
        <Row
          label="Tenant"
          description="Optional. Leave empty to allow work, school and personal accounts."
        >
          <Input
            value={tenant}
            placeholder="common"
            spellCheck={false}
            onChange={(event) => setTenant(event.target.value)}
          />
        </Row>
      ) : null}
      <div className="flex flex-wrap items-center justify-end gap-3 px-5 py-3">
        {error ? (
          <p role="alert" className="mr-auto text-[13px] text-cream">
            {error}
          </p>
        ) : null}
        {phase === "waiting" ? (
          <>
            <span className="text-[13px] text-cream-muted">
              Finish signing in in your browser…
            </span>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void cloudCancelSignIn(pendingName)}
            >
              Cancel
            </Button>
          </>
        ) : (
          <Button
            variant="primary"
            size="sm"
            disabled={!ready || phase !== "idle"}
            onClick={() => void add()}
          >
            Add and sign in
          </Button>
        )}
      </div>
    </Section>
  );
}

function ConflictRow(props: {
  conflict: CloudConflict;
  onResolved: () => void;
}) {
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const name = props.conflict.rel.split("/").pop() ?? props.conflict.rel;
  const resolve = async (choice: CloudConflictChoice) => {
    setWorking(true);
    setError(null);
    try {
      await cloudResolveConflict(props.conflict.localPath, choice);
      props.onResolved();
    } catch (cause) {
      setError(errorText(cause));
      setWorking(false);
    }
  };
  return (
    <Row
      label={name}
      description={error ?? `${props.conflict.remote}${props.conflict.rel}`}
    >
      <Button
        variant="outline"
        size="sm"
        disabled={working}
        onClick={() => void resolve("keep_theirs")}
      >
        Use cloud version
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={working}
        onClick={() => void resolve("keep_both")}
      >
        Keep both
      </Button>
      <Button
        variant="outline"
        size="sm"
        disabled={working}
        onClick={() => void resolve("keep_mine")}
      >
        Keep mine
      </Button>
    </Row>
  );
}
