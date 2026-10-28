import { cloudResolveConflict } from "@/native/cloud";
import type { OperationQueueSnapshot } from "@/native/ipc";
import type { OperationConflictPolicy } from "@/native/ipc/primitives";
import { operationQueueResolveConflict } from "@/native/transfers-tools";
import { errorText } from "@/shared/lib/format";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
  Button,
  cn,
} from "@/shared/ui";
import { useState } from "react";
import { explorerOpenPath } from "../../native";
import type {
  CompareImageState,
  CompareTextDiffState,
} from "../model/interfaces/workspace/ExplorerCompareDialog";
import { useOperationQueueStore } from "../store";
import { leftDiffKind, rightDiffKind } from "./compareDialog/compareDiff";
import { CompareDiffLine } from "./compareDialog/ComparePresentation";
import {
  loadCompareImagePreview,
  loadCompareTextDiff,
} from "./compareDialog/ComparePreview";
import { compareStyles } from "./ExplorerDesktopDialogStyles";

type Comparison =
  | { kind: "loading" }
  | { kind: "text"; diff: CompareTextDiffState }
  | { kind: "image"; images: CompareImageState }
  | { kind: "other" };

function fileName(path: string): string {
  return path.split(/[\\/]/).filter(Boolean).pop() ?? path;
}

/**
 * A cloud file changed here and in the cloud. The upload waits until the
 * person picks a version; comparing shows both side by side.
 */
export function CloudConflictPrompt({
  conflict,
}: {
  conflict: OperationQueueSnapshot["conflictDialog"];
}) {
  const [working, setWorking] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [comparison, setComparison] = useState<Comparison | null>(null);
  const mine = conflict.compareLocalPath;
  const theirs = conflict.compareCloudPath;
  const name = fileName(mine);

  const settle = async (run: () => Promise<OperationQueueSnapshot>) => {
    if (working) return;
    setWorking(true);
    setError(null);
    try {
      useOperationQueueStore.setState({ snapshot: await run() });
    } catch (cause) {
      setError(errorText(cause));
    } finally {
      setWorking(false);
    }
  };
  const resolve = (policy: OperationConflictPolicy) =>
    settle(() =>
      operationQueueResolveConflict(conflict.operationId, policy, false),
    );
  const useCloudVersion = () =>
    settle(() =>
      cloudResolveConflict(mine, "keep_theirs", conflict.operationId),
    );

  const compare = async () => {
    if (comparison) {
      setComparison(null);
      return;
    }
    setComparison({ kind: "loading" });
    const diff = await loadCompareTextDiff(mine, theirs);
    if (diff) return setComparison({ kind: "text", diff });
    const images = await loadCompareImagePreview(mine, theirs);
    setComparison(images ? { kind: "image", images } : { kind: "other" });
  };

  return (
    <AlertDialog open>
      <AlertDialogContent
        className={cn(comparison && "max-w-[min(960px,calc(100vw-2rem))]")}
      >
        <AlertDialogHeader>
          <AlertDialogTitle>“{name}” changed in the cloud too</AlertDialogTitle>
          <AlertDialogDescription>
            You changed this file, and the cloud copy changed after Kura
            downloaded it. Choose the version to keep. If you use the cloud
            version, your edit is kept in Kura’s conflicts folder.
          </AlertDialogDescription>
        </AlertDialogHeader>

        <div>
          <Button
            variant="outline"
            size="sm"
            disabled={working}
            onClick={() => void compare()}
          >
            {comparison ? "Hide comparison" : "Compare side by side"}
          </Button>
        </div>

        {comparison?.kind === "loading" ? (
          <p className="text-sm text-cream-muted">Loading both versions…</p>
        ) : null}
        {comparison?.kind === "text" ? (
          <div className={compareStyles.diffGrid}>
            <div className={compareStyles.diffPane}>
              <span className={compareStyles.diffPaneTitle}>Yours</span>
              {comparison.diff.rows.map((row) => (
                <CompareDiffLine
                  key={`mine:${row.id}`}
                  lineNumber={row.leftLine}
                  text={row.leftText}
                  kind={leftDiffKind(row)}
                />
              ))}
            </div>
            <div className={compareStyles.diffPane}>
              <span className={compareStyles.diffPaneTitle}>Cloud</span>
              {comparison.diff.rows.map((row) => (
                <CompareDiffLine
                  key={`cloud:${row.id}`}
                  lineNumber={row.rightLine}
                  text={row.rightText}
                  kind={rightDiffKind(row)}
                />
              ))}
            </div>
          </div>
        ) : null}
        {comparison?.kind === "image" ? (
          <div className={compareStyles.imageGrid}>
            {(
              [
                ["Yours", comparison.images.left],
                ["Cloud", comparison.images.right],
              ] as const
            ).map(([label, image]) => (
              <div className={compareStyles.imagePane} key={label}>
                <span className={compareStyles.imageMeta}>{label}</span>
                <span className={compareStyles.imageFrame}>
                  <img
                    className={compareStyles.imagePreview}
                    src={image.src}
                    alt={`${label} version`}
                  />
                </span>
              </div>
            ))}
          </div>
        ) : null}
        {comparison?.kind === "other" ? (
          <div className="flex flex-wrap items-center gap-2 text-sm text-cream-muted">
            Kura can’t show this file type side by side.
            <Button
              variant="outline"
              size="sm"
              onClick={() => void explorerOpenPath(mine)}
            >
              Open yours
            </Button>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void explorerOpenPath(theirs)}
            >
              Open the cloud version
            </Button>
          </div>
        ) : null}

        {error ? (
          <p role="alert" className="text-sm text-cream">
            {error}
          </p>
        ) : null}
        <AlertDialogFooter>
          <Button
            variant="ghost"
            disabled={working}
            onClick={() => void resolve("skip")}
          >
            Decide later
          </Button>
          <Button
            variant="secondary"
            disabled={working}
            onClick={() => void useCloudVersion()}
          >
            Use cloud version
          </Button>
          <Button
            variant="secondary"
            disabled={working}
            onClick={() => void resolve("keep_both")}
          >
            Keep both
          </Button>
          <Button disabled={working} onClick={() => void resolve("replace")}>
            Keep mine
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
