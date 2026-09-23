import { t } from "@sarmg/admin-ui/i18n";
import type { ConfigFieldDefinition, DeviceInfo } from "./api";

export type ConfigField = ConfigFieldDefinition & {
  category: ConfigCategory;
  label: string;
  inputKind: "text" | "integer" | "select";
  boolean: boolean;
  options: string[];
};

export const configCategories = [
  {id: "general", label: t("概况", "General")},
  {id: "input", label: t("输入", "Input")},
  {id: "audio-video", label: t("音频/视频", "Audio/Video")},
  {id: "network", label: t("网络", "Network")},
  {id: "files", label: t("配置文件", "Config files")},
  {id: "advanced", label: t("高级", "Advanced")},
  {id: "nvenc", label: t("NVIDIA NVENC 编码器", "NVIDIA NVENC encoder"), encoder: true},
  {id: "qsv", label: t("Intel Quick Sync 编码器", "Intel Quick Sync encoder"), encoder: true},
  {id: "amd", label: t("AMD AMF 编码器", "AMD AMF encoder"), encoder: true},
  {id: "vaapi", label: t("VA-API 编码器", "VA-API encoder"), encoder: true},
  {id: "vt", label: t("VideoToolbox 编码器", "VideoToolbox encoder"), encoder: true},
  {id: "software", label: t("软件编码器", "Software encoder"), encoder: true},
] as const;
export type ConfigCategory = typeof configCategories[number]["id"];

const categoryKeys: Partial<Record<ConfigCategory, readonly string[]>> = {
  general: ["sunshine_name", "locale", "notify_pre_releases", "system_tray"],
  input: ["controller", "keyboard", "mouse", "always_send_scancodes", "key_rightalt_to_key_win", "high_resolution_scrolling", "native_pen_touch", "back_button_timeout", "key_repeat_delay", "gamepad_driver", "gamepad", "ds4_back_as_touchpad_click", "motion_as_ds4", "touchpad_as_ds4", "virtualhid_randomize_mac"],
  "audio-video": ["encoder", "stream_audio"],
  network: ["fec_percentage", "max_bitrate"],
  files: ["min_log_level"],
  software: ["min_threads"],
};

function categoryForField(key: string): ConfigCategory {
  for (const category of configCategories) {
    if (categoryKeys[category.id]?.includes(key)) return category.id;
  }
  if (key.startsWith("dd_")) return "audio-video";
  if (key.startsWith("sw_")) return "software";
  for (const prefix of ["nvenc", "qsv", "amd", "vaapi", "vt"] as const) {
    if (key.startsWith(`${prefix}_`)) return prefix;
  }
  return "advanced";
}

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
  locale: t("界面语言", "Locale"), notify_pre_releases: t("预发布更新通知", "Prerelease notifications"), system_tray: t("系统托盘", "System tray"),
  encoder: t("编码器", "Encoder"), fec_percentage: t("前向纠错比例", "FEC percentage"), max_bitrate: t("最大码率", "Maximum bitrate"), stream_audio: t("串流音频", "Stream audio"),
  controller: t("控制器输入", "Controller input"), keyboard: t("键盘输入", "Keyboard input"), mouse: t("鼠标输入", "Mouse input"), always_send_scancodes: t("始终发送扫描码", "Always send scancodes"),
  key_rightalt_to_key_win: t("右 Alt 映射为 Windows 键", "Map Right Alt to Windows key"), high_resolution_scrolling: t("高分辨率滚动", "High resolution scrolling"), native_pen_touch: t("原生触控笔与触摸", "Native pen and touch"),
  back_button_timeout: t("返回键超时（毫秒）", "Back button timeout (ms)"), key_repeat_delay: t("按键重复延迟（毫秒）", "Key repeat delay (ms)"), gamepad_driver: t("虚拟手柄驱动", "Virtual gamepad driver"), gamepad: t("模拟手柄类型", "Emulated gamepad"),
  ds4_back_as_touchpad_click: t("DS4 返回键作为触控板点击", "DS4 Back as touchpad click"), motion_as_ds4: t("以 DS4 上报动作", "Report motion as DS4"), touchpad_as_ds4: t("以 DS4 上报触控板", "Report touchpad as DS4"), virtualhid_randomize_mac: t("随机化 VirtualHID MAC", "Randomize VirtualHID MAC"),
  dd_configuration_option: t("显示设备配置", "Display device configuration"), dd_resolution_option: t("分辨率模式", "Resolution mode"), dd_manual_resolution: t("手动分辨率", "Manual resolution"), dd_refresh_rate_option: t("刷新率模式", "Refresh-rate mode"), dd_manual_refresh_rate: t("手动刷新率", "Manual refresh rate"), dd_hdr_option: t("HDR 模式", "HDR mode"), dd_wa_hdr_toggle_delay: t("HDR 切换延迟（毫秒）", "HDR toggle delay (ms)"), dd_config_revert_delay: t("显示配置恢复延迟（毫秒）", "Display revert delay (ms)"), dd_config_revert_on_disconnect: t("断开时恢复显示配置", "Revert display configuration on disconnect"),
  sw_tune: t("软件编码调优", "Software encoder tune"), nvenc_twopass: t("NVENC 两遍编码", "NVENC two-pass mode"), nvenc_split_encode: t("NVENC 分帧编码", "NVENC split-frame encoding"), nvenc_realtime_hags: t("NVENC HAGS 实时优先级", "NVENC HAGS realtime priority"), nvenc_opengl_vulkan_on_dxgi: t("NVENC DXGI 上的 OpenGL/Vulkan", "NVENC OpenGL/Vulkan on DXGI"), nvenc_latency_over_power: t("NVENC 延迟优先", "NVENC latency over power"),
  qsv_preset: t("Quick Sync 预设", "Quick Sync preset"), qsv_coder: t("Quick Sync 熵编码", "Quick Sync coder"), qsv_slow_hevc: t("Quick Sync 慢速 HEVC", "Quick Sync slow HEVC"),
  amd_usage: t("AMD AMF 使用模式", "AMD AMF usage"), amd_rc: t("AMD AMF 码率控制", "AMD AMF rate control"), amd_quality: t("AMD AMF 质量", "AMD AMF quality"), amd_coder: t("AMD AMF 熵编码", "AMD AMF coder"), amd_preanalysis: t("AMD AMF 预分析", "AMD AMF preanalysis"), amd_vbaq: t("AMD AMF VBAQ", "AMD AMF VBAQ"), amd_enforce_hrd: t("AMD AMF 强制 HRD", "AMD AMF enforce HRD"), amd_max_au_size: t("AMD AMF 最大帧大小", "AMD AMF maximum AU size"),
  vaapi_quality: t("VA-API 质量", "VA-API quality"), vaapi_rc: t("VA-API 码率控制", "VA-API rate control"), vaapi_blbrc: t("VA-API 块级码率控制", "VA-API block-level rate control"), vaapi_strict_rc_buffer: t("VA-API 严格码率缓冲", "VA-API strict rate-control buffer"),
  vt_coder: t("VideoToolbox 熵编码", "VideoToolbox coder"), vt_software: t("VideoToolbox 软件编码", "VideoToolbox software encoding"), vt_realtime: t("VideoToolbox 实时模式", "VideoToolbox realtime mode"),
};

const prerequisites:Record<string,string>={software_encoder:t("需要软件编码器", "Requires the software encoder"),nvidia_encoder:t("需要 NVIDIA NVENC", "Requires NVIDIA NVENC"),intel_encoder:t("需要 Intel Quick Sync", "Requires Intel Quick Sync"),amd_encoder:t("需要 AMD AMF", "Requires AMD AMF"),vaapi_encoder:t("需要 VA-API 编码器", "Requires a VA-API encoder"),videotoolbox_encoder:t("需要 VideoToolbox 编码器", "Requires the VideoToolbox encoder"),installed_encoder:t("所选编码器必须已安装并可用", "The selected encoder must be installed and available"),interactive_desktop:t("需要交互式桌面会话", "Requires an interactive desktop session"),installed_gamepad_driver:t("需要相应的虚拟手柄驱动", "Requires the corresponding virtual gamepad driver"),virtualhid_driver:t("需要 Virtual HID Driver", "Requires Virtual HID Driver"),ds4_emulation:t("需要 DS4/DS5 模拟", "Requires DS4/DS5 emulation"),windows_display_control:t("仅适用于 Windows 显示控制", "Requires Windows display control"),manual_resolution:t("分辨率模式必须设为手动", "Resolution mode must be manual"),manual_refresh_rate:t("刷新率模式必须设为手动", "Refresh-rate mode must be manual")};

export function prerequisiteLabel(value:string|undefined):string|undefined{return value?(prerequisites[value]??value):undefined}

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
      category: categoryForField(field.key),
      label: labels[field.key] ?? field.key,
      inputKind: field.kind === "integer" ? "integer" : field.kind === "text" ? "text" : "select",
      boolean: field.kind === "boolean",
      options: field.kind === "boolean" ? ["true", "false"] : field.options ?? [],
    }));
}
