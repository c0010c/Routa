import {
  Autocomplete,
  Alert,
  Button,
  DialogActions,
  DialogContent,
  DialogTitle,
  Stack,
  TextField,
} from "@mui/material";
import { useState } from "react";
import { isValidationError, useModel, useSaveNodeGroup } from "../api/client";
import type { NodeGroup, NodeRef } from "../bindings";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { FormDialog } from "../components/FormDialog";

export function newNodeGroup(): NodeGroup {
  return { id: crypto.randomUUID(), name: "" };
}

export function NodeGroupDialog({
  group,
  onClose,
}: {
  group: NodeGroup;
  onClose: () => void;
}) {
  const model = useModel();
  const save = useSaveNodeGroup();
  const [name, setName] = useState(group.name);
  const options = [
    ...(model.data?.subscriptions ?? []).flatMap((s) =>
      s.nodes.map((n) => ({
        ref: { subscription_id: s.id, name: n.name } as NodeRef,
        label: `${n.name} · ${s.name}`,
      })),
    ),
    ...(model.data?.manual_nodes ?? []).map((n) => ({
      ref: { manual_id: n.id } as NodeRef,
      label: `${n.name} · ${"Tailscale" in n.kind ? "Tailscale" : "手动"}`,
    })),
  ];
  const key = (r: NodeRef) =>
    "manual_id" in r ? `m:${r.manual_id}` : `s:${r.subscription_id}/${r.name}`;
  const [members, setMembers] = useState<NodeRef[]>([
    ...(group.members ?? []),
    ...(model.data?.manual_nodes ?? [])
      .filter((n) => n.groups.includes(group.id))
      .map((n) => ({ manual_id: n.id })),
  ]);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });

  const submit = async () => {
    setErrs({ form: [] });
    try {
      await save.mutateAsync({ ...group, name: name.trim(), members });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const isEdit = model.data?.node_groups.some((g) => g.id === group.id);
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="xs">
      <DialogTitle>{isEdit ? "编辑分组" : "新建组"}</DialogTitle>
      <DialogContent>
        <Stack spacing={2} sx={{ mt: 1 }}>
          {errs.form.length > 0 && (
            <Alert severity="error" role="alert">
              {errs.form.map((m) => (
                <div key={m}>{m}</div>
              ))}
            </Alert>
          )}
          <Autocomplete
            multiple
            options={options}
            getOptionLabel={(o) => o.label}
            isOptionEqualToValue={(a, b) => key(a.ref) === key(b.ref)}
            value={options.filter((o) =>
              members.some((r) => key(r) === key(o.ref)),
            )}
            onChange={(_, v) => setMembers(v.map((o) => o.ref))}
            renderInput={(p) => <TextField {...p} label="分组成员" />}
          />
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
        <Button
          variant="contained"
          disabled={save.isPending || name.trim() === ""}
          onClick={() => void submit()}
        >
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}
