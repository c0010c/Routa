import { useQuery } from "@tanstack/react-query";
import { commands, type UpdateView } from "../bindings";

export function useUpdate() {
  return useQuery({ queryKey: ["update"], queryFn: () => commands.updateStatus() });
}

export async function checkUpdate(): Promise<UpdateView> {
  const r = await commands.updateCheck();
  if (r.status === "error") throw new Error("检查更新失败");
  return r.data;
}

export async function installUpdate(): Promise<void> {
  const r = await commands.updateInstall();
  if (r.status === "error") throw new Error(r.error);
}
