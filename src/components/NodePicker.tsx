import { Autocomplete, Box, TextField, Typography } from "@mui/material";
import { useMemo } from "react";
import { useModel, useStatus } from "../api/client";
import type { NodeRef } from "../bindings";

interface Option {
  ref: NodeRef;
  group: string;
}

const MISSING_GROUP = "已失效";
const key = (r: NodeRef) => `${r.subscription_id}/${r.name}`;

// Must be referentially stable: Autocomplete resets its input text whenever getOptionLabel changes.
const groupBy = (o: Option) => o.group;
const getOptionLabel = (o: Option) => o.ref.name;
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
        list.push({ ref, group: s.name });
      }
    }
    // Selected nodes that no longer exist stay visible so the user can see and remove them.
    for (const r of selected) if (!known.has(key(r))) list.push({ ref: r, group: MISSING_GROUP });
    return list;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [model.data, props.value]);

  const subName = (o: Option) => o.group;
  const latency = (o: Option) => {
    const d = status.data?.delays[`${subName(o)}/${o.ref.name}`];
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
            <span>{o.ref.name}</span>
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
