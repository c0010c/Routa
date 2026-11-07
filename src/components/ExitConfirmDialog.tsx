import {
  Button,
  Checkbox,
  Dialog,
  DialogActions,
  DialogContent,
  DialogContentText,
  DialogTitle,
  FormControlLabel,
} from "@mui/material";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";
import { commands } from "../bindings";
import { useDisableRoutingAndQuit, useSetExitPrompt } from "../api/procroute";
import { useApplyResult } from "./ApplyResultSnackbar";

/**
 * Shown when the tray's 退出 finds enabled block rules (Rust emits
 * "confirm-exit" with their names): quitting would cut those programs off.
 * Also reports why a tray action failed ("exit-error"): 停用进程分流并退出 did
 * not quit, or 启用 / 停用进程分流 could not be saved.
 */
export function ExitConfirmDialog() {
  const [names, setNames] = useState<string[] | null>(null);
  const [dontAsk, setDontAsk] = useState(false);
  const disableAndQuit = useDisableRoutingAndQuit();
  const setExitPrompt = useSetExitPrompt({ errorFeedback: "local" });
  const [busy, setBusy] = useState(false);
  const { showError } = useApplyResult();

  useEffect(() => {
    let disposed = false;
    const unlistens: (() => void)[] = [];
    const reg = (p: Promise<() => void>) =>
      p
        .then((u) => (disposed ? u() : unlistens.push(u)))
        .catch((e) => console.error("event subscription failed", e));
    reg(
      listen<string[]>("confirm-exit", (e) => {
        setNames(e.payload);
        setDontAsk(false);
        setBusy(false);
      }),
    );
    reg(listen<string>("exit-error", (e) => showError(e.payload)));
    return () => {
      disposed = true;
      unlistens.forEach((u) => u());
    };
  }, [showError]);

  // 仍然退出 always quits, so 不再提示 is saved right before it.
  const rememberChoice = async () => {
    if (!dontAsk) return;
    try {
      await setExitPrompt.mutateAsync(false);
    } catch (e) {
      // Not asking next time is secondary; the user still wants to quit.
      console.error("set_exit_prompt failed", e);
    }
  };

  const quit = async () => {
    setBusy(true);
    await rememberChoice();
    await commands.quitApp();
  };

  const disableThenQuit = async () => {
    setBusy(true);
    try {
      // 不再提示 is saved by the backend only once Routa really exits.
      await disableAndQuit.mutateAsync(dontAsk);
    } catch {
      // Routa did not quit (programs may still be blocked); the mutation shows
      // why. The dialog stays open so 仍然退出 is still available.
      setBusy(false);
    }
  };

  const close = () => setNames(null);

  return (
    <Dialog open={names !== null} onClose={close}>
      <DialogTitle>退出 Routa？</DialogTitle>
      <DialogContent>
        <DialogContentText>
          {`下面这些规则选了阻断，退出后对应程序会断网，直到 Routa 再次运行：${(names ?? []).join("、")}`}
        </DialogContentText>
        <FormControlLabel
          sx={{ mt: 1 }}
          control={<Checkbox checked={dontAsk} onChange={(e) => setDontAsk(e.target.checked)} />}
          label="不再提示"
        />
      </DialogContent>
      <DialogActions>
        <Button onClick={close}>取消</Button>
        <Button disabled={busy} onClick={() => void quit()}>
          仍然退出
        </Button>
        <Button variant="contained" disabled={busy} onClick={() => void disableThenQuit()}>
          停用进程分流并退出
        </Button>
      </DialogActions>
    </Dialog>
  );
}
