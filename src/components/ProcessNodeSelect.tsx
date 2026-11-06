import { useState } from "react";
import { Box, Button, CircularProgress, Typography } from "@mui/material";
import type { Model, NodeRef, ProcessRule } from "../bindings";
import { desktopApi, useDesktopMutation } from "../api/desktop";
import { errorMessage } from "../api/client";
import { ruleLabel } from "../lib/processRule";
import { nodeLabel } from "../lib/refs";
import { NodePickerDialog } from "./NodePickerDialog";

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
  return (
    <Box
      onClick={(e) => e.stopPropagation()}
      sx={{ minWidth: 195, maxWidth: 250 }}
    >
      <Button
        size="small"
        variant="outlined"
        color={problem ? "error" : "inherit"}
        disabled={save.isPending}
        aria-busy={save.isPending}
        onClick={() => setPicking(true)}
        sx={{ justifyContent: "flex-start", textTransform: "none", maxWidth: "100%" }}
      >
        <Box component="span" sx={{ overflow: "hidden", textOverflow: "ellipsis" }}>
          {text}
        </Box>
      </Button>
      {save.isPending && (
        <Typography role="status" variant="caption" color="text.secondary" sx={{ display: "block" }}>
          <CircularProgress size={10} sx={{ mr: 0.5 }} />
          正在应用…
        </Typography>
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
