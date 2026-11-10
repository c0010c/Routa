import { useRef, useState } from "react";
import { Box, Button, ButtonBase, CircularProgress, Tooltip, Typography } from "@mui/material";
import { Refresh } from "@mui/icons-material";
import type { Model, NodeRef, ProcessRule } from "../bindings";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { errorMessage, useStatus, useTestDelays } from "../api/client";
import { ruleExit } from "../lib/hidden";
import { DelayText, delayDescription } from "./DelayText";
import { ruleLabel } from "../lib/processRule";
import { nodeLabel } from "../lib/refs";
import { NodePickerDialog } from "./NodePickerDialog";

/** Read out by screen readers without taking space in the row. */
const VISUALLY_HIDDEN = {
  position: "absolute", width: 1, height: 1, p: 0, m: "-1px", overflow: "hidden",
  clip: "rect(0 0 0 0)", whiteSpace: "nowrap", border: 0,
} as const;

export const NODE_UNSET = "未选择节点，请选择节点";
export const NODE_BROKEN = "节点已失效，请更换节点";

/** 规则行里的节点：点击换节点；未选或失效时显示原因并用错误色。 */
export function ProcessNodeSelect({
  rule,
  model,
  reason,
}: {
  rule: ProcessRule;
  model: Model;
  reason?: string | null;
}) {
  const save = useDesktopMutation((node: NodeRef) =>
    desktopApi.changeNode(rule.id, node),
  );
  const status = useStatus(), test = useTestDelays();
  const measuring = useRef(false);
  const [picking, setPicking] = useState(false);
  const [failedTarget, setFailedTarget] = useState<NodeRef | null>(null);
  const [error, setError] = useState("");
  const change = async (node: NodeRef) => {
    setError("");
    try {
      await save.mutateAsync(node);
    } catch (e) {
      setFailedTarget(node);
      setError(errorMessage(e));
    }
  };
  const problem = !rule.node
    ? NODE_UNSET
    : reason === NODE_UNSET || reason === NODE_BROKEN
      ? reason
      : null;
  const text = problem ?? (rule.node ? nodeLabel(model, rule.node) : NODE_UNSET);
  // 与代理页一样按出口路径测延迟；测试对象显示节点名，不露出隐藏出口的内部名字。
  const s = status.data;
  const exit = problem ? undefined : ruleExit(model, rule);
  const pending = test.isPending && !!exit && !!test.variables?.[0]?.includes(exit.name);
  const delayProps = exit && s && {
    result: s.delays[exit.name],
    kind: "proxy" as const,
    target: text,
    testUrl: model.settings.delay_test_url,
    running: s.core === "Running",
    interfaceName: exit.interface
      ? s.interfaces.find((i) => i.guid === exit.interface)?.friendly_name ?? "网卡不可用"
      : "系统默认网卡",
    pending,
  };
  const measure = async () => {
    if (!exit || measuring.current) return;
    measuring.current = true;
    try {
      await test.mutateAsync([exit.name]);
    } catch {
      // The shared mutation reports measurement failures through the global feedback.
    } finally {
      measuring.current = false;
    }
  };
  return (
    <Box
      onClick={(e) => e.stopPropagation()}
      sx={{ minWidth: 195, maxWidth: 340 }}
    >
      <Button
        size="small"
        variant="outlined"
        color={problem ? "error" : "inherit"}
        // `loading` overlays the spinner on the kept label; a status line below would grow the rule row while it applies.
        loading={save.isPending}
        loadingIndicator={<CircularProgress size={14} color="inherit" />}
        aria-busy={save.isPending}
        onClick={() => setPicking(true)}
        sx={{ justifyContent: "flex-start", textTransform: "none", maxWidth: "100%" }}
      >
        <Box component="span" sx={{ overflow: "hidden", textOverflow: "ellipsis" }}>
          {text}
        </Box>
      </Button>
      {delayProps && (
        <Tooltip title={<Box sx={{ whiteSpace: "pre-line" }}>{delayDescription(delayProps)}{"\n"}点击重新测延迟</Box>}>
          <Box component="span" sx={{ display: "inline-flex", ml: 1, verticalAlign: "middle" }}>
            <ButtonBase
              aria-label={`测量 ${ruleLabel(rule)} 的代理延迟`}
              aria-busy={pending || undefined}
              disabled={test.isPending || !delayProps.running}
              onClick={() => void measure()}
              sx={{ borderRadius: 0.5, minHeight: 28, px: 0.5, gap: 0.5, "&:hover": { bgcolor: "action.hover" } }}
            >
              <DelayText {...delayProps} tooltip={false} emptyText="未测试" variant="card" />
              <Refresh sx={{ fontSize: 13, color: "text.secondary", visibility: pending ? "hidden" : undefined }} />
            </ButtonBase>
          </Box>
        </Tooltip>
      )}
      {save.isPending && (
        <Box component="span" role="status" sx={VISUALLY_HIDDEN}>
          正在应用…
        </Box>
      )}
      {error && (
        <Typography
          component="div"
          role="alert"
          variant="caption"
          color="error"
          sx={{ mt: 0.5 }}
        >
          {error}
          {failedTarget && (
            <Button size="small" onClick={() => void change(failedTarget)}>
              重试
            </Button>
          )}
        </Typography>
      )}
      {picking && (
        <NodePickerDialog
          title={`选择节点 · ${ruleLabel(rule)}`}
          value={rule.node ?? null}
          onClose={() => setPicking(false)}
          onPick={(node) => {
            setPicking(false);
            void change(node);
          }}
        />
      )}
    </Box>
  );
}
