import {
  Alert,
  Button,
  Checkbox,
  DialogActions,
  DialogContent,
  DialogTitle,
  FormControl,
  FormControlLabel,
  FormGroup,
  FormLabel,
  Stack,
  Switch,
  TextField,
} from "@mui/material";
import { useState } from "react";
import { isValidationError, useModel, useSaveManualNode } from "../api/client";
import type { ManualNode, TailscaleOpts } from "../bindings";
import { InterfacePicker } from "../components/InterfacePicker";
import { fieldErrors, type FieldErrors } from "../lib/validation";
import { FormDialog } from "../components/FormDialog";

export const EXIT_NODE_HELP = "auto:any 在当前内核下可能无法自动选中出口节点，建议填写出口节点的 IP 或名称";

const defaultHostname = (name: string) => `mihomo-${name}`;

export function newTailscaleNode(groups: string[] = []): ManualNode {
  const opts: TailscaleOpts = {
    hostname: defaultHostname(""),
    exit_node: "auto:any",
    ephemeral: false,
    udp: true,
    auth_key: null,
    interface: null,
  };
  return { id: crypto.randomUUID(), name: "", groups, kind: { Tailscale: opts } };
}

export function TailscaleDialog({ node, onClose }: { node: ManualNode; onClose: () => void }) {
  const model = useModel();
  const save = useSaveManualNode();
  const initial: TailscaleOpts =
    "Tailscale" in node.kind ? node.kind.Tailscale : (newTailscaleNode().kind as { Tailscale: TailscaleOpts }).Tailscale;

  const [name, setName] = useState(node.name);
  const [hostname, setHostname] = useState(initial.hostname);
  // The hostname tracks the name until the user edits it by hand.
  // Existing instances never change hostname implicitly: a new hostname registers as a new tailnet machine.
  const isExisting = !!model.data?.manual_nodes.some((n) => n.id === node.id);
  const [hostTouched, setHostTouched] = useState(initial.hostname !== defaultHostname(node.name));
  const [exitNode, setExitNode] = useState(initial.exit_node);
  const [ephemeral, setEphemeral] = useState(initial.ephemeral);
  const [udp, setUdp] = useState(initial.udp);
  const [authKey, setAuthKey] = useState(initial.auth_key ?? "");
  const [iface, setIface] = useState(initial.interface);
  const [groups, setGroups] = useState<string[]>(node.groups);
  const [errs, setErrs] = useState<FieldErrors>({ form: [] });

  const changeName = (v: string) => {
    setName(v);
    if (!hostTouched && !isExisting) setHostname(defaultHostname(v));
    setErrs((x) => ({ ...x, name: undefined }));
  };
  const toggleGroup = (id: string) => setGroups((gs) => (gs.includes(id) ? gs.filter((g) => g !== id) : [...gs, id]));

  const submit = async () => {
    setErrs({ form: [] });
    const opts: TailscaleOpts = {
      hostname: hostname.trim(),
      exit_node: exitNode.trim(),
      ephemeral,
      udp,
      auth_key: authKey.trim() === "" ? null : authKey.trim(),
      interface: iface,
    };
    try {
      await save.mutateAsync({ ...node, name: name.trim(), groups, kind: { Tailscale: opts } });
      onClose();
    } catch (e) {
      if (isValidationError(e)) setErrs(fieldErrors(e.errors));
    }
  };

  const nodeGroups = model.data?.node_groups ?? [];
  const isEdit = model.data?.manual_nodes.some((n) => n.id === node.id);
  const canSave = name.trim() !== "" && hostname.trim() !== "" && exitNode.trim() !== "";
  return (
    <FormDialog onClose={onClose} fullWidth maxWidth="sm">
      <DialogTitle>{isEdit ? "编辑 Tailscale 实例" : "新建 Tailscale 实例"}</DialogTitle>
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
            value={name}
            onChange={(e) => changeName(e.target.value)}
            error={!!errs.name}
            helperText={errs.name}
          />
          <TextField
            size="small"
            label="hostname"
            value={hostname}
            onChange={(e) => {
              setHostname(e.target.value);
              setHostTouched(true);
            }}
          />
          <TextField
            size="small"
            label="exit-node"
            value={exitNode}
            onChange={(e) => setExitNode(e.target.value)}
            helperText={exitNode.trim() === "auto:any" ? EXIT_NODE_HELP : undefined}
          />
          <FormControlLabel control={<Switch checked={ephemeral} onChange={(e) => setEphemeral(e.target.checked)} />} label="ephemeral" />
          <FormControlLabel control={<Switch checked={udp} onChange={(e) => setUdp(e.target.checked)} />} label="UDP" />
          <TextField
            size="small"
            label="auth-key"
            type="password"
            value={authKey}
            onChange={(e) => setAuthKey(e.target.value)}
            helperText="可留空，之后在 Tailscale 页登录"
            autoComplete="off"
          />
          <InterfacePicker value={iface} onChange={setIface} />
          {nodeGroups.length > 0 && (
            <FormControl component="fieldset">
              <FormLabel component="legend">所属组</FormLabel>
              <FormGroup row>
                {nodeGroups.map((g) => (
                  <FormControlLabel
                    key={g.id}
                    control={<Checkbox checked={groups.includes(g.id)} onChange={() => toggleGroup(g.id)} />}
                    label={g.name}
                  />
                ))}
              </FormGroup>
            </FormControl>
          )}
        </Stack>
      </DialogContent>
      <DialogActions>
        <Button onClick={onClose}>取消</Button>
        <Button variant="contained" disabled={save.isPending || !canSave} onClick={() => void submit()}>
          保存
        </Button>
      </DialogActions>
    </FormDialog>
  );
}
