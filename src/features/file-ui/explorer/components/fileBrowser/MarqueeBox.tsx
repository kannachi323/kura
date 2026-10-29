import { fileBrowserStyles } from "../FileBrowserStyles";
import type { MarqueeRect } from "./useMarqueeSelection";

export function MarqueeBox(props: { rect: MarqueeRect }) {
  return (
    <div
      aria-hidden="true"
      className={fileBrowserStyles.marquee}
      style={{
        left: props.rect.left,
        top: props.rect.top,
        width: props.rect.right - props.rect.left,
        height: props.rect.bottom - props.rect.top,
      }}
    />
  );
}
