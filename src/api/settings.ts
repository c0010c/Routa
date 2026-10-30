import { useMutation, useQueryClient } from "@tanstack/react-query";
import { commands, type Model, type Settings } from "../bindings";
import { unwrap } from "./client";

export function settingsSaveMessage(error: unknown) {
  const message = error instanceof Error ? error.message : "";
  return [
    "程序正在退出，设置未保存",
    "开机自启未能保存，请重试",
    "设置未能写入磁盘，请重试",
    "设置未能保存，开机自启未能恢复，请检查系统启动项",
  ].includes(message) ? message : "未能保存，请重试";
}

/** Merge only the submitted fields into the committed model, never a form's drafts. */
export function useSettingsSave() {
  const qc = useQueryClient();
  return useMutation({
    mutationFn: async (patch: Partial<Settings>) => {
      await qc.cancelQueries({ queryKey: ["model"] });
      const model = qc.getQueryData<Model>(["model"]);
      if (!model) throw new Error("设置尚未加载，请重试");
      const settings = { ...model.settings, ...patch };
      const outcome = unwrap(await commands.saveSettings(settings));
      await qc.cancelQueries({ queryKey: ["model"] });
      qc.setQueryData<Model>(["model"], (current) => current ? { ...current, settings } : current);
      void qc.invalidateQueries({ queryKey: ["status"] });
      return outcome;
    },
    retry: false,
    scope: { id: "settings-save" },
  });
}
