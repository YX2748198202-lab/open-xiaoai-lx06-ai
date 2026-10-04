# 架构说明

## 总体结构

项目将“共享 AI 能力”和“设备固件差异”分开处理。

```text
原生唤醒 / ASR
      ↓
平台适配层
      ↓
standalone-ai 主流程
  ├─ 去重和状态管理
  ├─ 本地意图路由
  ├─ OpenAI-compatible API
  ├─ 多轮历史和 System Prompt
  ├─ 原生 TTS
  └─ 原生 XiaoAI fallback
```

## 共享部分

`standalone-ai` 是 Tokio 异步 Rust 程序，以下逻辑由 LX06 和 OH2P 共用：

- OpenAI-compatible Chat Completions 请求；
- System Prompt 拼接；
- 多轮历史；
- 设备控制与普通问答路由；
- 请求超时、错误分类和额度提醒；
- 原生 XiaoAI fallback；
- TTS 调用后的状态和日志处理。

## 平台适配层

设备差异集中在 `src/platform.rs`，主流程不直接散落 LX06 专用路径。

当前平台档案包括：

### LX06

- 设备：Xiaomi 小爱音箱 Pro；
- 架构：ARMv7 hard-float；
- Target：`armv7-unknown-linux-gnueabihf`；
- 已实测 ASR 日志：`/tmp/log/messages`；
- 原生服务：`/etc/init.d/mico_aivs_lab`；
- TTS：`mibrain.text_to_speech`；
- fallback：`mibrain.ai_service`；
- 当前状态：已完成实机验证。

### OH2P

- 设备：小米智能音箱 Pro；
- 预计架构：AArch64；
- Target：`aarch64-unknown-linux-gnu`；
- 固件静态分析发现 AIVS/MiBrain 相关组件；
- 当前原生服务和 ubus 方法按固件布局建立了候选档案；
- 当前状态：实验性适配，尚未完成真实设备验证。

OH2P 的候选接口不能被静态固件分析视为已经确认。实际部署前必须通过真实设备验证 ASR、TTS、fallback、自启动和长期稳定性。

## 输入处理

程序监控设备原生日志，并解析两类输入：

1. 原生 JSON `SpeechRecognizer / RecognizeResult` 事件；
2. LX06 已实测的 `speech_recognizer.asr=...` 日志格式，其中 `nlp=true` 作为交接标记。

可以使用 `ASR_LOG_PATHS` 覆盖默认日志路径：

```ini
ASR_LOG_PATHS=/tmp/mico_aivs_lab/instruction.log,/tmp/log/messages
```

## 路由

设备控制使用本地关键词规则识别，并交给原生小爱独占处理；普通问答、解释、比较、故事和知识问题发送到配置的兼容 API。

这种路由不是完整的自然语言意图识别，复杂或歧义表达可能需要继续完善规则。

## 安全边界

项目只设计为向 `/data` 写入程序、配置和启动脚本，不主动修改 boot、system、vendor 等固件分区。

项目不包含本地大模型权重，也不会提取或复制小米云端官方 System Prompt。