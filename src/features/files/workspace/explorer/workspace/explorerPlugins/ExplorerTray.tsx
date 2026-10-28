import { openTerminalAtPath } from "../../../native";
import { reportSystemError } from "@/features/activity";
import { useWorkspaceStore } from "@/features/workspace";
import { IconButton, toolbarIconProps } from "@/shared/ui";
import { ArrowRightLeft, Settings, Terminal } from "lucide-react";
import { useCallback, useState } from "react";

export function ExplorerTray(props: {
  terminalEnabled: boolean;
  terminalPath: string;
  onOpenSettings?: () => void;
}) {
  const [errorMessage, setErrorMessage] = useState("");
  const openTerminal = useCallback(() => {
    if (!props.terminalEnabled) return;
    setErrorMessage("");
    void openTerminalAtPath(props.terminalPath).catch((error: unknown) => {
      setErrorMessage("Terminal could not be opened. Try again.");
      reportSystemError({
        error,
        scope: "files:terminal",
        title: "Terminal could not be opened",
      });
    });
  }, [props.terminalEnabled, props.terminalPath]);

  return (
    <>
      {errorMessage ? (
        <span role="alert" className="text-xs text-cream-muted">
          {errorMessage}
        </span>
      ) : null}
      <IconButton
        label="Open terminal"
        tooltip={false}
        title={
          props.terminalEnabled
            ? "Open terminal"
            : "Terminal unavailable for this view"
        }
        disabled={!props.terminalEnabled}
        onClick={openTerminal}
      >
        <Terminal {...toolbarIconProps} />
      </IconButton>
      {props.onOpenSettings ? (
        <IconButton
          label="Settings"
          tooltip={false}
          onClick={props.onOpenSettings}
        >
          <Settings {...toolbarIconProps} />
        </IconButton>
      ) : null}
    </>
  );
}

/** Opens the Transfers tab, or returns to it when it is already open. */
export function TransfersButton() {
  return (
    <IconButton
      label="Transfers"
      onClick={() => useWorkspaceStore.getState().openDestination("transfers")}
    >
      <ArrowRightLeft {...toolbarIconProps} />
    </IconButton>
  );
}
