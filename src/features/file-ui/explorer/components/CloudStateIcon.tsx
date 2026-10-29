import type { CloudCopyState } from "@/native/ipc";
import { cn } from "@/shared/ui";
import { CircleCheck, Cloud, CloudUpload, TriangleAlert } from "lucide-react";

const states = {
  online: { icon: Cloud, label: "In the cloud only" },
  downloaded: { icon: CircleCheck, label: "Downloaded" },
  modified: { icon: CloudUpload, label: "Changed here; uploading" },
  conflict: { icon: TriangleAlert, label: "Changed here and in the cloud" },
} as const;

/** Whether a cloud file is downloaded, waiting to upload, or conflicting. */
export function CloudStateIcon(props: { state?: CloudCopyState }) {
  if (!props.state) return null;
  const { icon: Icon, label } = states[props.state];
  return (
    <span
      title={label}
      aria-label={label}
      role="img"
      className={cn(
        "inline-flex shrink-0 items-center",
        props.state === "conflict" ? "text-cream-bright" : "text-cream-muted",
      )}
    >
      <Icon size={13} strokeWidth={1.9} aria-hidden="true" />
    </span>
  );
}
