import { Button } from "@/shared/ui";
import { Component, type ErrorInfo, type ReactNode } from "react";

/** Keeps one broken tab from taking down the window. */
export class RenderErrorBoundary extends Component<
  { children: ReactNode; scope?: "workspace" | "tab" },
  { error: Error | null }
> {
  state: { error: Error | null } = { error: null };

  static getDerivedStateFromError(error: Error) {
    return { error };
  }

  componentDidCatch(error: Error, info: ErrorInfo) {
    if (import.meta.env.DEV)
      console.error("Render failed", error, info.componentStack);
  }

  render() {
    if (!this.state.error) return this.props.children;
    const tab = this.props.scope === "tab";
    return (
      <section className="grid h-full min-h-0 min-w-0 content-center justify-items-center gap-3 bg-charcoal-bg p-8 text-center text-cream">
        <h1 className="m-0 text-[22px] font-semibold">
          {tab ? "This tab could not be loaded" : "Something went wrong"}
        </h1>
        <p className="m-0 max-w-[680px] text-cream-muted [overflow-wrap:anywhere]">
          {this.state.error.message}
        </p>
        <Button
          variant="outline"
          onClick={() =>
            tab ? this.setState({ error: null }) : window.location.reload()
          }
        >
          {tab ? "Try again" : "Reload Kura"}
        </Button>
      </section>
    );
  }
}
