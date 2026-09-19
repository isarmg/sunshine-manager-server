import { t } from "@sarmg/admin-ui/i18n";
import type { ConfigFieldDefinition, DeviceInfo } from "./api";

export type ConfigField = ConfigFieldDefinition & {
  label: string;
  inputKind: "text" | "integer" | "select";
  boolean: boolean;
  options: string[];
};

const labels: Record<string, string> = {
  sunshine_name: t("Sunshine 名称", "Sunshine name"),
  min_log_level: t("日志级别", "Log level"),
  qp: t("量化参数 QP", "Quantization parameter (QP)"),
  hevc_mode: t("HEVC 模式", "HEVC mode"),
  av1_mode: t("AV1 模式", "AV1 mode"),
  min_threads: t("最少线程数", "Minimum threads"),
  sw_preset: t("软件编码预设", "Software encoding preset"),
  nvenc_preset: t("NVENC 预设", "NVENC preset"),
  nvenc_vbv_increase: t("NVENC VBV 增量", "NVENC VBV increase"),
  nvenc_spatial_aq: t("NVENC 空间自适应量化", "NVENC spatial adaptive quantization"),
  nvenc_h264_cavlc: t("NVENC H.264 CAVLC", "NVENC H.264 CAVLC"),
};

export function fieldsForDevice(
  definitions: ConfigFieldDefinition[],
  device: DeviceInfo,
): ConfigField[] {
  const capabilities = device.capabilities;
  if (capabilities === null) return [];
  return definitions
    .filter((field) =>
      capabilities.managed_fields.includes(field.key)
      && field.supported_sunshine_versions.includes(capabilities.sunshine_version)
      && field.operating_systems.includes(capabilities.os))
    .map((field) => ({
      ...field,
      label: labels[field.key] ?? field.key,
      inputKind: field.kind === "integer" ? "integer" : field.kind === "text" ? "text" : "select",
      boolean: field.kind === "boolean",
      options: field.kind === "boolean" ? ["true", "false"] : field.options ?? [],
    }));
}
