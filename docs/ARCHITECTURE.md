# 架构说明

## 进程

`standalone-ai` 是 Tokio 异步 Rust 程序，主要组件：

- 文件监控：读取 LX06 原生 ASR 输出；
- 本地路由：区分设备控制与普通问答；
- HTTP 客户端：请求 OpenAI-compatible Chat Completions；
- 历史状态：在内存中保留多轮上下文；
- TTS 适配：调用 `mibrain.text_to_speech`；
- 降级状态：API 失败时切回原生 XiaoAI。

## 不做的事情

- 不修改 boot、system、vendor 等固件分区；
- 不在设备上运行大模型权重；
- 不替换原生 ASR 和 TTS；
- 不宣称掌握小米云端官方 Prompt。
