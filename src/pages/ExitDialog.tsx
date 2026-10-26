import {
  Alert,
  Box,
  Button,
  Dialog,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  InputLabel,
  MenuItem,
  Select,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { useState } from "react";
import { isValidationError, useModel, useSaveExit, useStatus } from "../api/client";
import type { Exit, ExitKind, GroupStrategy, NodeRef } from "../bindings";
import { InterfacePicker } from "../components/InterfacePicker";
import { NodePicker } from "../components/NodePicker";
import { fieldErrors, type FieldErrors } from "../lib/validation";

type KindType = "Node" | "Group" | "Direct";

const STRATEGIES: { value: GroupStrategy; label: string }[] = [
  { value: "UrlTest", label: "自动测速（url-test）" },
  { value: "Fallback", label: "故障转移（fallback）" },
  { value: "LoadBalance", label: "负载均衡（load-balance）" },
  { value: "Select", label: "手动选择" },
];

const refKey = (r: NodeRef) => `${r.subscription_id}/${r.name}`;

export function newExit(): Exit {
  return { id: crypto.randomUUID(), name: "", kind: "Direct", interface: null };
}

function kindType(k: ExitKind): KindType {
  return k === "Direct" ? "Direct" : "Node" in k ? "Node" : "Group";
}

export function ExitDialog({ exit, onClose }: { exit: Exit; onClose: () => void }) {
  const model = useModel();
  const status = useStatus();
  const save = useSaveExit();
  const [name, setName] = useState(exit.name);
  const [type, setType] = useState<KindType>(kindType(exit.kind));
  const [node, setNode] = useState<NodeRef | null>(
    typeof exit.kind === "object" && "Node" in exit.kind ? exit.kind.Node.node : null,
  );
  const [strategy, setStrategy] = useState<GroupStrategy>(
    typeof exit.kind === "object" && "Group" in exit.kind ? exit.kind.Group.strategy : "UrlTest",
  );
  const [members, setMembers] = useState<NodeRef[]>(
    typeof exit.kind === "object" && "Group" in exit.kind ? exit.kind.Group.members : [],
  );
  const [selected] = useState<NodeRef | null>(
    typeof exit.kind === "object" && "Group" in exit.kind ? exit.kind.Group.selected : null,
  );
  const [iface, setIface] = useState(exit.interface);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });
  const [nodeError, setNodeError] = useState<string>();

  const missing = status.data?.health.exits[exit.id]?.missing_members ?? [];
  const missingKeys = new Set(missing.map(refKey));
  const staleInForm = members.filter((m) => missingKeys.has(refKey(m)));

  const submit = async () => {
    let kind: ExitKind;
    if (type === "Direct") kind = "Direct";
    else if (type === "Node") {
      if (!node) {
        setNodeError("请选择节点");
        return;
      }
      kind = { Node: { node } };
    } else {
      kind = {
        Group: {
          strategy,
          members,
          selected: selected && members.some((m) => refKey(m) === refKey(selected)) ? selected : null,
        },
      };
    }
    setNodeError(undefined);
    setErrs({ form: [] });
    try {
      await save.mutateAsync({ ...exit, name, kind, interface: iface });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const isEdit = model.data?.exits.some((x) => x.id === exit.id);
  return (
    <Dialog open onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isEdit ? "编辑出口" : "新建出口"}</DialogTitle>
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
            label="名称"
            size="small"
            value={name}
            onChange={(e) => {
              setName(e.target.value);
              setErrs((x) => ({ ...x, name: undefined }));
            }}
            error={!!errs.name}
            helperText={errs.name}
          />
          <FormControl size="small">
            <InputLabel id="e-type">类型</InputLabel>
            <Select labelId="e-type" label="类型" value={type} onChange={(e) => setType(e.target.value as KindType)}>
              <MenuItem value="Node">节点</MenuItem>
              <MenuItem value="Group">策略组</MenuItem>
              <MenuItem value="Direct">直连</MenuItem>
            </Select>
          </FormControl>
          {type === "Node" && (
            <>
              <NodePicker value={node} onChange={setNode} />
              {nodeError && (
                <Typography color="error" variant="caption">
                  {nodeError}
                </Typography>
              )}
            </>
          )}
          {type === "Group" && (
            <>
              <FormControl size="small">
                <InputLabel id="e-strategy">策略</InputLabel>
                <Select
                  labelId="e-strategy"
                  label="策略"
                  value={strategy}
                  onChange={(e) => setStrategy(e.target.value as GroupStrategy)}
                >
                  {STRATEGIES.map((s) => (
                    <MenuItem key={s.value} value={s.value}>
                      {s.label}
                    </MenuItem>
                  ))}
                </Select>
              </FormControl>
              <NodePicker multiple label="成员" value={members} onChange={setMembers} />
              {staleInForm.length > 0 && (
                <Box sx={{ display: "flex", alignItems: "center", gap: 2 }}>
                  <Typography color="error" variant="body2">
                    失效成员：{staleInForm.map((m) => m.name).join("、")}
                  </Typography>
                  <Button
                    size="small"
                    color="error"
                    onClick={() => setMembers(members.filter((m) => !missingKeys.has(refKey(m))))}
                  >
                    移除失效成员
                  </Button>
                </Box>
              )}
            </>
          )}
          <InterfacePicker value={iface} onChange={setIface} />
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={save.isPending} onClick={() => void submit()}>
          保存
        </Button>
      </DialogActions>
    </Dialog>
  );
}
