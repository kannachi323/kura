import type { CloudProviderKind } from "@/native/cloud";
import { cn } from "@/shared/ui";
import { cloudProviderGuide } from "./providers";

export function ProviderLogo(props: {
  kind: CloudProviderKind;
  size?: number;
  className?: string;
}) {
  const guide = cloudProviderGuide(props.kind);
  const size = props.size ?? 16;
  return (
    <img
      src={guide.logo}
      alt=""
      aria-hidden="true"
      width={size}
      height={size}
      draggable={false}
      className={cn("inline-block shrink-0 object-contain", props.className)}
    />
  );
}
