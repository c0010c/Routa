import { Alert, Button, DialogActions, DialogContent, DialogTitle, Stack, TextField } from "@mui/material";
import { useState } from "react";
import { isValidationError, useModel, useSaveNodeGroup } from "../api/client";
import type { NodeGroup } from "../bindings";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { FormDialog } from "../components/FormDialog";

export function newNodeGroup(): NodeGroup {
  return { id: crypto.randomUUID(), name: "" };
}

export function NodeGroupDialog({ group, onClose }: { group: NodeGroup; onClose: () => void }) {
  const model = useModel();
  const save = useSaveNodeGroup();
  const [name, setName] = useState(group.name);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });

  const submit = async () => {
    setErrs({ form: [] });
    try {
      await save.mutateAsync({ ...group, name: name.trim() });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const isEdit = model.data?.node_groups.some((g) => g.id === group.id);
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="xs">
      <DialogTitle>{isEdit ? "重命名组" : "新建组"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 1 }}>
          {errs.form.length > 0 && (
            <Alert severity="error" role="alert">
              {errs.form.map((m) => (
                <div key={m}>{m}</div>
              ))}
            </Alert>
          )}
          <TextField
            size="small"
            label="名称"
            autoFocus
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setErrs((x) => ({ ...x, name: undefined }));
            }}
            error={!!errs.name}
            helperText={errs.name}
          />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={save.isPending || name.trim() === ""} onClick={() => void submit()}>
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}
