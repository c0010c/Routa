import {
  Alert, Button, Dialog, DialogActions, DialogContent, DialogTitle,
  List, ListItem, ListItemText, Stack, Typography,
} from "@mui/material";
import { useState } from "react";
import { Link as RouterLink } from "react-router-dom";
import { errorMessage, isValidationError, useDeleteExit, useModel } from "../api/client";
import { collectExitUsages } from "../lib/refs";
import { fieldErrors } from "../lib/validation";

export function ExitDeleteDialog({ exitId, onClose }: { exitId: string; onClose: () => void }) {
  const model = useModel();
  const del = useDeleteExit({ errorFeedback: "local" });
  const [error, setError] = useState("");
  const [checking, setChecking] = useState(false);
  const [needsRecheck, setNeedsRecheck] = useState(false);
  const exit = model.data?.exits.find((e) => e.id === exitId);
  const usages = model.data ? collectExitUsages(model.data, exitId) : [];
  const busy = del.isPending || checking;

  const recheck = async (afterRejection = false) => {
    setChecking(true);
    setNeedsRecheck(true);
    setError("");
    try {
      const result = await model.refetch();
      if (!result.isSuccess) throw result.error;
      if (afterRejection && collectExitUsages(result.data, exitId).length === 0) {
        setError("配置引用发生变化，暂时无法删除。请重新检查后重试。");
      } else {
        setNeedsRecheck(false);
      }
    } catch (e) {
      setError(`未能读取最新配置，请重新检查后再删除。${errorMessage(e)}`);
    } finally {
      setChecking(false);
    }
  };

  const confirm = async () => {
    if (busy || !exit || usages.length > 0 || needsRecheck || model.isError) return;
    setError("");
    try {
      await del.mutateAsync(exitId);
      onClose();
    } catch (e) {
      if (isValidationError(e)) {
        if (e.errors.some((item) => "DanglingRef" in item)) {
          await recheck(true);
        } else {
          const fields = fieldErrors(e.errors);
          setError([fields.name, fields.port, ...fields.form].filter(Boolean).join("\n"));
        }
      } else {
        setError(`删除失败：${errorMessage(e)}`);
      }
    }
  };

  const readFailed = model.isError;
  const canDelete = !!exit && usages.length === 0 && !needsRecheck && !readFailed;
  return (
    <Dialog open fullWidth maxWidth="sm" onClose={() => !busy && onClose()} aria-labelledby="exit-delete-title">
      <DialogTitle id="exit-delete-title">
        {usages.length > 0 && exit ? `暂时无法删除「${exit.name}」` : "确认删除"}
      </DialogTitle>
      <DialogContent>
        <Stack spacing={2}>
          {checking ? (
            <Typography role="status">配置引用发生变化，正在重新检查…</Typography>
          ) : readFailed ? (
            <Alert severity="error">{error || `未能读取最新配置，请重新检查后再删除。${errorMessage(model.error)}`}</Alert>
          ) : !model.data ? (
            <Typography>加载中…</Typography>
          ) : !exit ? (
            <Typography>该出口已不存在</Typography>
          ) : usages.length > 0 ? (
            <>
              <Typography>以下配置正在使用此出口，请先处理后再删除。</Typography>
              {(["listener", "subscription"] as const).map((kind) => {
                const items = usages.filter((u) => u.kind === kind);
                if (items.length === 0) return null;
                const label = kind === "listener" ? "监听器" : "订阅";
                return (
                  <section key={kind} aria-label={`${label}引用`}>
                    <Typography variant="subtitle2">{label}（{items.length}）</Typography>
                    <List disablePadding>
                      {items.map((u) => {
                        const params = new URLSearchParams({
                          [kind === "listener" ? "highlight" : "editSubscription"]: u.id,
                          resolveExit: exitId,
                        });
                        return (
                          <ListItem key={u.id} disableGutters>
                            <ListItemText
                              primary={u.name}
                              secondary={kind === "listener" ? "用途：流量出口" : "用途：拉取订阅失败时使用的备用出口"}
                              slotProps={{ primary: { style: { overflowWrap: "anywhere" } } }}
                            />
                            <Button
                              component={RouterLink}
                              to={`${kind === "listener" ? "/" : "/nodes"}?${params}`}
                              aria-label={`处理${label}「${u.name}」`}
                              disabled={busy}
                              sx={{ flexShrink: 0 }}
                            >
                              前往处理
                            </Button>
                          </ListItem>
                        );
                      })}
                    </List>
                  </section>
                );
              })}
              <Typography variant="body2" color="text.secondary">
                监听器可切换到其他出口，暂时不用时可选择“不接”。订阅可切换备用出口，不再需要时可选择“不使用”。
              </Typography>
            </>
          ) : (
            <Typography>确定删除出口「{exit.name}」吗？</Typography>
          )}
          {error && !readFailed && !checking && <Alert severity="error" sx={{ whiteSpace: "pre-line" }}>{error}</Alert>}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose} disabled={busy}>{canDelete ? "取消" : "关闭"}</Button>
        {(needsRecheck || readFailed) && <Button disabled={busy} onClick={() => void recheck()}>重新检查</Button>}
        {canDelete && (
          <Button color="error" variant="contained" disabled={busy} onClick={() => void confirm()}>
            {del.isPending ? "删除中…" : "删除"}
          </Button>
        )}
      </DialogActions>
    </Dialog>
  );
}
