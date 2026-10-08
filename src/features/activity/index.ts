import { createElement, useEffect } from "react";

export interface SystemErrorActivityInput {
  /** Background refreshes are logged only; user actions also render inline. */
  intent?: "background" | "user-action";
  title: string;
  onRetry?: () => void;
  error?: unknown;
  body?: string;
  scope: string;
}

/** Records a diagnostic in the developer console. Kura has no activity feed. */
export function reportSystemError(
  input: SystemErrorActivityInput,
): string | null {
  const body = systemErrorMessage(input.body?.trim() || input.error);
  console.warn(`[${input.scope}] ${input.title}: ${body}`);
  return null;
}

/** Foreground failures render where they happen; background reports render nothing. */
export function SystemErrorActivity({
  intent,
  body,
  error,
  scope,
  title,
  onRetry,
}: SystemErrorActivityInput) {
  const message = body?.trim() || systemErrorMessage(error);

  useEffect(() => {
    reportSystemError({ intent, body: message, scope, title });
  }, [intent, message, scope, title]);

  if (intent === "background") return null;
  return createElement(
    "div",
    {
      role: "alert",
      className:
        "flex flex-wrap items-center gap-2 px-3 py-2 text-sm text-cream-muted",
    },
    createElement("span", null, `${title}. ${systemErrorMessage(message)}`),
    onRetry
      ? createElement(
          "button",
          {
            type: "button",
            onClick: onRetry,
            className: "min-h-11 px-2 underline hover:text-cream-bright",
          },
          "Retry",
        )
      : null,
  );
}

export function systemErrorMessage(error: unknown): string {
  const message =
    error instanceof Error
      ? error.message
      : typeof error === "string"
        ? error
        : "";
  const trimmed = message.trim();
  if (!trimmed) return "Something went wrong. Try again.";
  return trimmed.length > 240 ? `${trimmed.slice(0, 237)}…` : trimmed;
}
