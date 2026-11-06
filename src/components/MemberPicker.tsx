import { Autocomplete, Box, TextField, Typography } from "@mui/material";
import { useMemo } from "react";
import { useModel, useStatus } from "../api/client";
import type { ExitMember } from "../bindings";
import { delayText } from "../lib/format";
import { expandMembers, isGroupMember, memberKey, nodeLabel, proxyNameOf } from "../lib/refs";

interface Option {
  member: ExitMember;
  group: string;
  label: string;
}

const SUB_GROUPS = "订阅组";
const CUSTOM_GROUPS = "自建组";
const MANUAL_GROUP = "手动节点";
const MISSING_GROUP = "已失效";

// Must be referentially stable: Autocomplete resets its input text whenever getOptionLabel changes.
const groupBy = (o: Option) => o.group;
const getOptionLabel = (o: Option) => o.label;
const isOptionEqualToValue = (a: Option, b: Option) => memberKey(a.member) === memberKey(b.member);
const getOptionKey = (o: Option) => memberKey(o.member);

export function MemberPicker({
  value,
  onChange,
  label,
}: {
  value: ExitMember[];
  onChange: (v: ExitMember[]) => void;
  label?: string;
}) {
  const model = useModel();
  const status = useStatus();

  const options = useMemo<Option[]>(() => {
    const m = model.data;
    if (!m) return [];
    const list: Option[] = [];
    for (const s of m.subscriptions) {
      list.push({
        member: { group: { Subscription: s.id } },
        group: SUB_GROUPS,
        label: `${s.name}（整组 · ${s.nodes.length}）`,
      });
    }
    for (const g of m.node_groups) {
      const count = expandMembers(m,[{group:{Custom:g.id}}]).length;
      list.push({ member: { group: { Custom: g.id } }, group: CUSTOM_GROUPS, label: `${g.name}（整组 · ${count}）` });
    }
    for (const s of m.subscriptions) {
      for (const n of s.nodes) {
        list.push({ member: { subscription_id: s.id, name: n.name }, group: s.name, label: n.name });
      }
    }
    for (const n of m.manual_nodes) {
      list.push({ member: { manual_id: n.id }, group: MANUAL_GROUP, label: n.name });
    }
    return list;
  }, [model.data]);

  const optionFor = (mem: ExitMember): Option => {
    const found = options.find((o) => memberKey(o.member) === memberKey(mem));
    if (found) return found;
    // Selected members that no longer exist stay visible so the user can see and remove them.
    const text = isGroupMember(mem) ? "（已删除的组）" : model.data ? nodeLabel(model.data, mem) : "";
    return { member: mem, group: MISSING_GROUP, label: text };
  };

  const latency = (o: Option) => {
    if (isGroupMember(o.member) || !model.data) return "";
    const key = proxyNameOf(model.data, o.member);
    const d = key ? status.data?.delays[key] : undefined;
    if (!d) return "";
    return delayText(d);
  };

  return (
    <Autocomplete
      multiple
      options={options}
      groupBy={groupBy}
      getOptionLabel={getOptionLabel}
      isOptionEqualToValue={isOptionEqualToValue}
      getOptionKey={getOptionKey}
      noOptionsText="没有可选成员"
      value={value.map(optionFor)}
      onChange={(_, v) => onChange(v.map((o) => o.member))}
      renderOption={(p, o) => {
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
      }}
      renderInput={(params) => <TextField {...params} label={label ?? "成员"} size="small" />}
    />
  );
}
