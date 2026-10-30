import { fieldErrors } from "../lib/validation";
import type { ValidationError } from "../bindings";
import { useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useQueryClient } from "@tanstack/react-query";
import {
  Alert,
  Button,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { FormDialog } from "../components/FormDialog";
import { useApplyResult } from "../components/ApplyResultSnackbar";
import { errorMessage } from "../api/client";
import type { ApplyOutcome, ManualNode, ParsedProxy } from "../bindings";
import { tailscaleFromRaw } from "./ManualNodeDialog";
export function BatchNodeDialog({ onClose }: { onClose: () => void }) {
  const [text, setText] = useState("");
  const [preview, setPreview] = useState<ParsedProxy[] | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const qc = useQueryClient();
  const feedback = useApplyResult();
  const parse = async () => {
    setBusy(true);
    setError("");
    try {
      setPreview(
        await invoke<ParsedProxy[]>("parse_proxy_yaml_batch", { text }),
      );
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };
  const save = async () => {
    if (!preview) return;
    setBusy(true);
    try {
      const nodes: ManualNode[] = preview.map((p) => {
        const raw = p.raw as Record<string, string>;
        const name = String(raw.name);
        return {
          id: crypto.randomUUID(),
          name,
          groups: [],
          kind:
            raw.type === "tailscale"
              ? { Tailscale: tailscaleFromRaw(raw, name) }
              : { Proxy: { raw: p.raw } },
        };
      });
      const outcome = await invoke<ApplyOutcome>("save_manual_nodes", {
        nodes,
      });
      feedback.show(outcome);
      await qc.invalidateQueries({ queryKey: ["model"] });
      await qc.invalidateQueries({ queryKey: ["status"] });
      onClose();
    } catch (e) {
      setError(
        Array.isArray(e)
          ? Object.values(fieldErrors(e as ValidationError[])).flat().filter(Boolean).join("；")
          : errorMessage(e),
      );
    } finally {
      setBusy(false);
    }
  };
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>导入 YAML</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ pt: 1 }}>
          {error && <Alert severity="error">{error}</Alert>}
          <TextField
            label="节点 YAML"
            multiline
            minRows={10}
            value={text}
            onChange={(e) => {
              setText(e.target.value);
              setPreview(null);
            }}
            placeholder="支持单个节点、节点列表或 proxies 配置"
          />
          <Button disabled={busy || !text.trim()} onClick={() => void parse()}>
            解析预览
          </Button>
          {preview && (
            <>
              <Typography>共 {preview.length} 个节点</Typography>
              {preview.map((p, i) => (
                <Typography key={i} variant="body2">
                  {String((p.raw as Record<string, unknown>).name)} ·{" "}
                  {String((p.raw as Record<string, unknown>).type)}
                  {p.stripped.length > 0
                    ? ` · 已移除 ${p.stripped.join("、")}`
                    : ""}
                </Typography>
              ))}
            </>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button
          variant="contained"
          disabled={busy || !preview}
          onClick={() => void save()}
        >
          全部保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}
