import { Box, Button, Paper, Stack, Typography } from "@mui/material";
import { useState, type ReactNode } from "react";
import { useNavigate } from "react-router-dom";
import { isValidationError, useSaveExit, useSaveListener } from "../api/client";
import type { Exit, Model } from "../bindings";
import { newListener } from "../pages/ListenerDialog";
import { newExit } from "../pages/ExitDialog";
import { applyOutcomeMessages } from "../lib/format";
import { fieldErrors } from "../lib/validation";
import { useApplyResult } from "./ApplyResultSnackbar";

/** The card is shown until at least one listener has an exit. */
export function needsGettingStarted(m: Model): boolean {
  return !m.listeners.some((l) => l.exit !== null);
}

/** `base`, or `base 2`, `base 3`… — the first one not in `taken`. */
export function uniqueName(base: string, taken: string[]): string {
  const set = new Set(taken);
  if (!set.has(base)) return base;
  for (let i = 2; ; i++) if (!set.has(`${base} ${i}`)) return `${base} ${i}`;
}

function nodeCount(m: Model): number {
  return m.subscriptions.reduce((n, s) => n + s.nodes.length, 0) + m.manual_nodes.length;
}

function Step({
  n,
  done,
  waiting,
  title,
  desc,
  action,
}: {
  n: number;
  done: boolean;
  waiting: boolean;
  title: ReactNode;
  desc: string;
  action: ReactNode;
}) {
  return (
    <Paper variant="outlined" sx={{ p: 1.25, display: "flex", alignItems: "center", gap: 1.5 }}>
      <Box
        aria-label={done ? `第 ${n} 步已完成` : `第 ${n} 步`}
        sx={{
          width: 24,
          height: 24,
          borderRadius: "50%",
          flex: "none",
          display: "flex",
          alignItems: "center",
          justifyContent: "center",
          fontSize: 12,
          color: done ? "success.contrastText" : "primary.contrastText",
          bgcolor: done ? "success.main" : waiting ? "action.disabled" : "primary.main",
        }}
      >
        {done ? "✓" : n}
      </Box>
      <Box sx={{ flex: 1 }}>
        <Typography sx={{ color: waiting ? "text.secondary" : undefined }}>{title}</Typography>
        <Typography variant="caption" sx={{ color: "text.secondary" }}>
          {desc}
        </Typography>
      </Box>
      {action}
    </Paper>
  );
}

export function GettingStarted({
  model: m,
  onNewExit,
  onNewListener,
}: {
  model: Model;
  onNewExit: () => void;
  onNewListener: () => void;
}) {
  const navigate = useNavigate();
  const saveExit = useSaveExit();
  const saveListener = useSaveListener();
  const apply = useApplyResult();
  const [busy, setBusy] = useState(false);
  const nodes = nodeCount(m);
  const hasExit = m.exits.length > 0;

  const quickStart = async () => {
    setBusy(true);
    try {
      // Reuse an existing direct exit on the system default interface rather than piling up copies.
      let exit: Exit | undefined = m.exits.find((e) => e.kind === "Direct" && e.interface === null);
      if (!exit) {
        const taken = [...m.exits.map((e) => e.name), ...m.subscriptions.map((s) => s.name)];
        exit = { ...newExit(), name: uniqueName("直连", taken) };
        await saveExit.mutateAsync(exit);
      }
      const listener = {
        ...newListener(m.listeners),
        name: uniqueName("默认", m.listeners.map((l) => l.name)),
        exit: exit.id,
      };
      const outcome = await saveListener.mutateAsync(listener);
      // Apply problems have their own snackbar; don't cover them.
      if (applyOutcomeMessages(outcome).length === 0)
        apply.showMessage(`已创建：端口 ${listener.port} 走「${exit.name}」，可以用了`, "success");
      navigate(`/?highlight=${listener.id}`);
    } catch (e) {
      // Other errors are already shown by the mutation hooks.
      if (isValidationError(e)) {
        const f = fieldErrors(e.errors);
        const lines = [f.name, f.port, ...f.form].filter((x): x is string => !!x);
        apply.showError(["快速开始失败，请按步骤手动创建：", ...lines].join("\n"));
      }
    } finally {
      setBusy(false);
    }
  };

  return (
    <Paper sx={{ p: 2 }}>
      <Stack direction="row" sx={{ justifyContent: "space-between", alignItems: "center", mb: 1.5, gap: 2 }}>
        <Box>
          <Typography sx={{ fontWeight: 600 }}>开始使用：三步让一个端口能上网</Typography>
          <Typography variant="caption" sx={{ color: "text.secondary" }}>
            监听器（本地端口）→ 出口（流量从哪出去）→ 节点 / 直连
          </Typography>
        </Box>
        <Button variant="contained" disabled={busy} onClick={() => void quickStart()}>
          快速开始：建一个直连端口
        </Button>
      </Stack>
      <Stack spacing={1}>
        <Step
          n={1}
          done={nodes > 0}
          waiting={false}
          title={
            <>
              准备节点{" "}
              <Typography component="span" variant="caption" sx={{ color: "text.secondary" }}>
                （可选，只用直连可跳过）
              </Typography>
            </>
          }
          desc={nodes > 0 ? `已有 ${m.subscriptions.length} 个订阅，${nodes} 个节点` : "添加订阅或手动节点"}
          action={
            <Button variant="outlined" size="small" onClick={() => navigate("/nodes")}>
              去节点页
            </Button>
          }
        />
        <Step
          n={2}
          done={hasExit}
          waiting={false}
          title="新建出口"
          desc={hasExit ? `已有 ${m.exits.length} 个出口` : "决定流量从哪出去：直连、某个节点，或一组节点自动选"}
          action={
            <Button variant="outlined" size="small" onClick={onNewExit}>
              新建出口
            </Button>
          }
        />
        <Step
          n={3}
          done={false}
          waiting={!hasExit}
          title="新建监听器并接上出口"
          desc="设备连这个端口上网，流量按接的出口走"
          action={
            <Button variant="outlined" size="small" onClick={onNewListener}>
              新建监听器
            </Button>
          }
        />
      </Stack>
    </Paper>
  );
}
