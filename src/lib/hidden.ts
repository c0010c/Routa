import type { Exit, Listener, Model } from "../bindings";

/** 进程分流自动管理的隐藏监听器端口区间，与后端 HIDDEN_PORT_RANGE 保持一致。 */
export const HIDDEN_PORT_MIN = 37800;
export const HIDDEN_PORT_MAX = 37999;

/** 缺少 hidden 字段视为 false。 */
export function visibleListeners(model: Model): Listener[] {
  return model.listeners.filter((l) => !l.hidden);
}

export function visibleExits(model: Model): Exit[] {
  return model.exits.filter((e) => !e.hidden);
}

export function isHiddenExitId(model: Model, id: string): boolean {
  return model.exits.some((e) => e.id === id && !!e.hidden);
}
