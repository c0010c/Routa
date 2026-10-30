import { Alert, Button, Dialog, DialogActions, DialogContent, DialogTitle } from "@mui/material";
import { useState } from "react";
import { isValidationError } from "../api/client";
import { fieldErrors } from "../lib/validation";

interface Props {
  text: string;
  onConfirm: () => Promise<unknown>;
  onClose: () => void;
  /** Defaults to 确认删除 / 删除; other destructive actions pass their own wording. */
  title?: string;
  confirmLabel?: string;
}

/** Confirms a delete; validation failures (e.g. still referenced) are shown inside the dialog. */
export function ConfirmDeleteDialog({ text, onConfirm, onClose, title = "确认删除", confirmLabel = "删除" }: Props) {
  const [errors, setErrors] = useState<string[]>([]);
  const [busy, setBusy] = useState(false);
  const confirm = async () => {
    setBusy(true);
    try {
      await onConfirm();
      onClose();
    } catch (e) {
      if (isValidationError(e)) {
        const f = fieldErrors(e.errors);
        setErrors([...(f.name ? [f.name] : []), ...(f.port ? [f.port] : []), ...f.form]);
      }
      setBusy(false);
    }
  };
  return (
    <Dialog open onClose={onClose} sx={{"& .MuiDialog-paper":{height:340,maxHeight:"calc(100dvh - 40px)",display:"flex",flexDirection:"column"},"& .MuiDialogTitle-root, & .MuiDialogActions-root":{flexShrink:0},"& .MuiDialogContent-root":{flex:1,overflowY:"auto"}}}>
      <DialogTitle>{title}</DialogTitle>
      <DialogContent>
        {text}
        {errors.length > 0 && (
          <Alert severity="error" role="alert" sx={{ mt: 2 }}>
            {errors.map((m) => (
              <div key={m}>{m}</div>
            ))}
          </Alert>
        )}
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button color="error" variant="contained" disabled={busy} onClick={() => void confirm()}>
          {confirmLabel}
        </Button>
      </DialogActions>
    </Dialog>
  );
}
