import { t } from "@xcss/admin-ui/i18n";
import { isSnapshot, type Command, type Operation } from "./api";

export const configurationActions = {
  read_config: "sunshine.config.read",
  patch_config: "sunshine.config.patch",
  save_config: "sunshine.config.save",
  restart: "sunshine.restart",
} as const;
export type ConfigurationCommand = keyof typeof configurationActions;
export type ConfigurationActivity = { command: ConfigurationCommand; operation: Operation };

export function isConfigurationCommand(command: Command): command is Extract<Command, { kind: ConfigurationCommand }> {
  return Object.hasOwn(configurationActions, command.kind);
}

export function configurationFeedback(activity: ConfigurationActivity): { message: string; error: boolean } {
  const { command, operation } = activity;
  const read = command === "read_config";
  const restart = command === "restart";
  const mismatch = () => ({ message: t("配置操作回执不匹配，请检查日志核对结果。", "The configuration operation receipt does not match. Check the result in Logs."), error: true });
  if (operation.action !== configurationActions[command]) return mismatch();
  if (operation.state === "pending" || operation.state === "running") {
    return { message: read ? t("正在读取配置…", "Reading configuration…")
      : restart ? t("重启请求已下发，等待客户端执行回执。", "The restart request was dispatched. Waiting for the client receipt.")
      : operation.state === "pending" ? t("配置保存请求已排队，等待客户端执行。", "The configuration save request is queued. Waiting for the client.")
      : t("正在保存配置，等待客户端执行回执。", "Saving configuration. Waiting for the client receipt."), error: false };
  }
  if (operation.state === "resolved") return { message: t("该操作已人工核对，请查看日志中的结论。", "This operation was manually reconciled. See the conclusion in Logs."), error: false };
  if (operation.state === "unknown") {
    return { message: read ? t("配置读取结果未确认，请检查日志后重新刷新页面。", "The configuration read is unconfirmed. Check Logs, then refresh the page.")
      : restart ? t("Sunshine 重启结果未确认，请检查日志并核对进程状态，勿直接重复重启。", "The Sunshine restart is unconfirmed. Check Logs and the process state before repeating it.")
      : t("配置保存结果未确认，请检查日志并核对客户端实际配置。", "The configuration save is unconfirmed. Check Logs and the client's actual configuration."), error: true };
  }
  if (operation.result?.kind === "conflict") return { message: t("配置或设备状态发生冲突，请刷新后核对。", "Configuration or device state conflict. Refresh and review the latest state."), error: true };
  if (operation.state === "failed" || operation.state === "dead_letter") {
    return { message: read ? t("配置读取失败，请刷新页面后重试。", "Reading configuration failed. Refresh the page to retry.")
      : restart ? t("Sunshine 重启失败，请检查日志。", "The Sunshine restart failed. Check Logs.")
      : t("配置保存失败，请检查日志。", "Saving configuration failed. Check Logs."), error: true };
  }
  if (operation.state !== "succeeded" || !isSnapshot(operation.result?.snapshot)) return mismatch();
  if (read) return operation.result?.kind === "config_read"
    ? { message: t("配置已读取，运行时生效待验证。", "Configuration read; runtime effect needs verification."), error: false } : mismatch();
  if (restart) return operation.result?.kind === "restart_acknowledged" && operation.result.process_generation
    ? { message: t("Sunshine 已重启，运行时生效待验证。", "Sunshine restarted; runtime effect needs verification."), error: false } : mismatch();
  if (operation.result?.kind !== "config_saved") return mismatch();
  return { message: operation.result.snapshot.effectiveness === "awaiting_restart"
    ? t("配置已保存，等待管理员重启。", "Configuration saved; waiting for an administrator to restart.")
    : t("配置已保存，运行时生效待验证。", "Configuration saved; runtime effect needs verification."), error: false };
}
