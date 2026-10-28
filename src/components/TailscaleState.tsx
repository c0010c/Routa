import { Alert, Button, Chip, type ChipProps, Stack, Typography } from "@mui/material";
import { useQueryClient } from "@tanstack/react-query";
import { useCallback, useState } from "react";
import { useRetryTailscaleExitNodes, useStatus } from "../api/client";
import { commands, type DelayResult, type ManualNode, type RuntimeStatus, type TailscaleOpts, type TailscaleState } from "../bindings";
import { MANUAL_PREFIX } from "../lib/refs";
import { findSelf, useTailnetPeers } from "../lib/tailnet";

export const tailscaleProxyName = (node: ManualNode) => `${MANUAL_PREFIX}/${node.name}`;

/** Login state of a Tailscale node from the runtime status (keyed by proxy name). */
export function useTailscaleState(node: ManualNode): TailscaleState {
  const status = useStatus();
  return status.data?.tailscale?.[tailscaleProxyName(node)] ?? "Unknown";
}

/** Mirrors the backend's EXIT_NODE_MAX_RETRIES. */
export const EXIT_NODE_MAX_RETRIES = 5;

/** mihomo could not apply the node's exit node; `retrying` = the automatic retry under way, null once they are used up. */
export type ExitNodeIssue = { reason: string; retrying: number | null };

function exitNodeIssue(st: RuntimeStatus | undefined, name: string): ExitNodeIssue | null {
  const reason = st?.tailscale_exit_errors?.[name];
  return reason === undefined ? null : { reason, retrying: st?.tailscale_exit_retrying?.[name] ?? null };
}

export function useExitNodeIssue(node: ManualNode): ExitNodeIssue | null {
  const status = useStatus();
  return exitNodeIssue(status.data, tailscaleProxyName(node));
}

/** The exit node failure: automatic retry progress, or the final failure with a manual retry. */
export function ExitNodeIssueAlert({
  issue,
  onChangeExitNode,
  withRetry = true,
}: {
  issue: ExitNodeIssue;
  onChangeExitNode?: () => void;
  withRetry?: boolean;
}) {
  const retry = useRetryTailscaleExitNodes();
  if (issue.retrying !== null) {
    return (
      <Alert severity="info">
        出口节点这次没设置上，正在自动重启实例重试（第 {issue.retrying}/{EXIT_NODE_MAX_RETRIES} 次）…
      </Alert>
    );
  }
  return (
    <Alert
      severity="error"
      action={
        <Stack direction="row" spacing={1}>
          {withRetry && (
            <Button size="small" color="inherit" disabled={retry.isPending} onClick={() => retry.mutate()}>
              重试
            </Button>
          )}
          {onChangeExitNode && (
            <Button size="small" color="inherit" onClick={onChangeExitNode}>
              更换出口节点
            </Button>
          )}
        </Stack>
      }
    >
      出口节点没有生效：{issue.reason}。内核启动实例时偶尔会设置失败，已自动重试 {EXIT_NODE_MAX_RETRIES} 次仍未成功，可以再试一次；一直失败的话，请检查出口节点是否在线并已在管理后台批准。
    </Alert>
  );
}

function stateChip(s: TailscaleState, ip: string | null): { label: string; color: ChipProps["color"] } {
  if (s === "Unknown") return { label: "状态未知", color: "default" };
  if ("NeedsLogin" in s) return { label: "待登录", color: "warning" };
  if ("LoggedIn" in s) {
    const shown = s.LoggedIn.ip ?? ip;
    return { label: shown ? `已登录 · ${shown}` : "已登录", color: "success" };
  }
  return { label: `登录失败：${s.Failed.reason}`, color: "error" };
}

/**
 * Compact login state label; the tailnet IP comes from the system client when
 * it knows the instance. `compact` leaves the IP out (narrow table cells).
 */
export function TailscaleStateChip({ node, compact = false }: { node: ManualNode; compact?: boolean }) {
  const s = useTailscaleState(node);
  const loggedIn = s !== "Unknown" && "LoggedIn" in s;
  const { peers } = useTailnetPeers(loggedIn && !compact);
  const hostname = (node.kind as { Tailscale: TailscaleOpts }).Tailscale?.hostname ?? "";
  const ip = loggedIn && !compact ? (findSelf(peers, hostname)?.ips[0] ?? null) : null;
  const { label, color } = stateChip(compact && loggedIn ? { LoggedIn: { ip: null } } : s, ip);
  return <Chip size="small" label={label} color={color} sx={{ alignSelf: "flex-start" }} />;
}

/** The authorization link with open/copy actions, while one is pending. */
export function TailscaleLoginLink({ node, hideChip = false }: { node: ManualNode; hideChip?: boolean }) {
  const s = useTailscaleState(node);
  // Copy result per URL, so a fresh URL offers 复制链接 again.
  const [copy, setCopy] = useState<{ url: string; ok: boolean } | null>(null);
  const [openError, setOpenError] = useState<string | null>(null);
  const needsLogin = s !== "Unknown" && "NeedsLogin" in s;
  const url = needsLogin ? s.NeedsLogin.url : null;
  return (
    <Stack spacing={1} sx={{ alignItems: "flex-start" }}>
      {!hideChip && <TailscaleStateChip node={node} />}
      {needsLogin && url === null && (
        <Typography variant="body2" color="text.secondary">
          正在获取授权链接…若长时间没有出现，请检查网络或该实例的网卡设置
        </Typography>
      )}
      {url !== null && (
        <>
          <Typography
            variant="body2"
            sx={{ userSelect: "text", wordBreak: "break-all", fontFamily: "monospace", bgcolor: "action.hover", px: 1, py: 0.5, borderRadius: 1 }}
          >
            {url}
          </Typography>
          <Stack direction="row" spacing={1} sx={{ alignItems: "center", flexWrap: "wrap" }}>
            <Button
              size="small"
              variant="contained"
              onClick={async () => {
                const r = await commands.openLoginUrl(url);
                setOpenError(r.status === "ok" ? null : r.error);
              }}
            >
              在浏览器中打开
            </Button>
            <Button
              size="small"
              variant="outlined"
              onClick={async () => {
                try {
                  // navigator.clipboard may be missing: that throws synchronously.
                  await navigator.clipboard.writeText(url);
                  setCopy({ url, ok: true });
                } catch (e) {
                  console.error("copy failed", e);
                  setCopy({ url, ok: false });
                }
              }}
            >
              {copy?.url === url && copy.ok ? "已复制" : "复制链接"}
            </Button>
            {copy?.url === url && !copy.ok && (
              <Typography variant="body2" color="error">
                复制失败，请手动选择链接
              </Typography>
            )}
          </Stack>
          {openError && (
            <Typography variant="body2" color="error">
              {openError}，请复制链接后在浏览器中打开
            </Typography>
          )}
        </>
      )}
    </Stack>
  );
}

export type CheckState =
  | { phase: "idle" }
  | { phase: "running"; elapsedSecs: number; retrying: number | null }
  | { phase: "done"; result: DelayResult; issue: ExitNodeIssue | null };

/** A restarted instance passed traffic through its exit node after 16–65 s in measurements; allow for retries. */
export const CHECK_TIMEOUT_MS = 120_000;
export const CHECK_RETRY_MS = 3000;

/**
 * Delay-tests a Tailscale instance until it answers. After a save mihomo
 * recreates the instance, which needs a while to come up, and may fail to
 * apply the exit node (the backend then retries by itself), so failures are
 * retried until the deadline; a failure the backend gave up on ends the check.
 */
export function useTailscaleCheck(node: ManualNode) {
  const qc = useQueryClient();
  const [state, setState] = useState<CheckState>({ phase: "idle" });
  const name = tailscaleProxyName(node);
  const run = useCallback(async () => {
    const start = Date.now();
    const elapsed = () => Math.round((Date.now() - start) / 1000);
    for (;;) {
      const issueNow = exitNodeIssue(qc.getQueryData<RuntimeStatus>(["status"]), name);
      setState({ phase: "running", elapsedSecs: elapsed(), retrying: issueNow?.retrying ?? null });
      const r = await commands.testDelays([name]);
      const result: DelayResult = (r.status === "ok" ? r.data[0]?.[1] : undefined) ?? { ms: null, tested_at: 0, error: "NoCore" };
      const issue = exitNodeIssue(qc.getQueryData<RuntimeStatus>(["status"]), name);
      const gaveUp = issue !== null && issue.retrying === null;
      if (result.ms !== null || gaveUp || Date.now() - start >= CHECK_TIMEOUT_MS) {
        setState({ phase: "done", result, issue: result.ms !== null ? null : issue });
        return;
      }
      setState({ phase: "running", elapsedSecs: elapsed(), retrying: issue?.retrying ?? null });
      await new Promise((ok) => setTimeout(ok, CHECK_RETRY_MS));
    }
  }, [name, qc]);
  return { state, run };
}
