import { Autocomplete, Box, TextField, Typography } from "@mui/material";
import { useMemo } from "react";
import { useModel, useStatus } from "../api/client";
import type { NodeRef } from "../bindings";
import { isSubRef, nodeKey, proxyNameOf } from "../lib/refs";

interface Option {
  ref: NodeRef;
  group: string;
  label: string;
}

const MISSING_GROUP = "已失效";
const MANUAL_GROUP = "手动节点";
const key = nodeKey;

// Must be referentially stable: Autocomplete resets its input text whenever getOptionLabel changes.
const groupBy = (o: Option) => o.group;
const getOptionLabel = (o: Option) => o.label;
const isOptionEqualToValue = (a: Option, b: Option) => key(a.ref) === key(b.ref);
const getOptionKey = (o: Option) => key(o.ref);

type Props = { label?: string } & (
  | { multiple?: false; value: NodeRef | null; onChange: (v: NodeRef | null) => void }
  | { multiple: true; value: NodeRef[]; onChange: (v: NodeRef[]) => void }
);

export function NodePicker(props: Props) {
  const model = useModel();
  const status = useStatus();
  const selected: NodeRef[] = props.multiple ? props.value : props.value ? [props.value] : [];

  const options = useMemo<Option[]>(() => {
    const list: Option[] = [];
    const known = new Set<string>();
    for (const s of model.data?.subscriptions ?? []) {
      for (const n of s.nodes) {
        const ref = { subscription_id: s.id, name: n.name };
        known.add(key(ref));
        list.push({ ref, group: s.name, label: n.name });
      }
    }
    for (const n of model.data?.manual_nodes ?? []) {
      const ref = { manual_id: n.id };
      known.add(key(ref));
      list.push({ ref, group: MANUAL_GROUP, label: n.name });
    }
    // Selected nodes that no longer exist stay visible so the user can see and remove them.
    for (const r of selected) {
      if (!known.has(key(r))) {
        list.push({ ref: r, group: MISSING_GROUP, label: isSubRef(r) ? r.name : "（已删除）" });
      }
    }
    return list;
  }, [model.data, props.value]);

  const latency = (o: Option) => {
    const pn = model.data ? proxyNameOf(model.data, o.ref) : null;
    const d = pn ? status.data?.delays[pn] : undefined;
    if (!d) return "";
    return d.ms === null ? "超时" : `${d.ms} ms`;
  };
  const toOption = (r: NodeRef) => options.find((o) => key(o.ref) === key(r)) as Option;
  const common = {
    options,
    groupBy,
    getOptionLabel,
    isOptionEqualToValue,
    getOptionKey,
    noOptionsText: "没有可选节点",
    renderOption: (p: React.HTMLAttributes<HTMLLIElement>, o: Option) => {
      const { key: k, ...rest } = p as React.HTMLAttributes<HTMLLIElement> & { key?: string };
      return (
        <li key={k} {...rest}>
          <Box sx={{ display: "flex", width: "100%", justifyContent: "space-between", gap: 2 }}>
            <span>{o.label}</span>
            <Typography component="span" variant="body2" color="text.secondary">
              {latency(o)}
            </Typography>
          </Box>
        </li>
      );
    },
    renderInput: (params: React.ComponentProps<typeof TextField>) => (
      <TextField {...params} label={props.label ?? "节点"} size="small" />
    ),
  };

  if (props.multiple) {
    const onChange = props.onChange;
    return (
      <Autocomplete
        {...common}
        multiple
        value={props.value.map(toOption)}
        onChange={(_, v) => onChange(v.map((o) => o.ref))}
      />
    );
  }
  const onChange = props.onChange;
  return (
    <Autocomplete
      {...common}
      value={props.value ? toOption(props.value) : null}
      onChange={(_, v) => onChange(v ? v.ref : null)}
    />
  );
}
