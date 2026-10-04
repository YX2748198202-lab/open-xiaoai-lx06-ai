# 配置说明

## 设备档案

```ini
DEVICE_MODEL=LX06
```

可选值：

- `LX06`：Xiaomi 小爱音箱 Pro，ARMv7，当前已完成实机验证。
- `OH2P`：小米智能音箱 Pro，AArch64，当前为实验性适配。

不同设备必须使用匹配架构的二进制。`OH2P` 配置项只会启用 OH2P 平台档案，不代表设备兼容性已经验收。

如果真实设备的 ASR 日志路径与默认候选不同，可以覆盖：

```ini
ASR_LOG_PATHS=/tmp/mico_aivs_lab/instruction.log,/tmp/log/messages
```

不配置时使用设备档案中的默认候选路径。

## API

```ini
BASE_URL=https://api.xiaomimimo.com/v1
API_KEY=CHANGE_ME
MODEL=mimo-v2.6-flash
```

`BASE_URL` 可以是任何兼容 OpenAI Chat Completions 的服务。程序会自动追加 `/chat/completions`。

不要把真实 API Key 写入源码、公开仓库、Issue、日志或截图。

## Prompt

```ini
SYSTEM_PROMPT=你的System Prompt
```

`SYSTEM_PROMPT` 必须保持为一行。需要换行时使用字面量 `\\n`。

Prompt 是用户自定义内容，不是小米官方 Prompt。Prompt 会作为 `system` 消息发送给兼容 API。

## 性能

```ini
HISTORY_MAX_LENGTH=10
MAX_RESPONSE_CHARS=1200
REQUEST_TIMEOUT_SECONDS=25
ABORT_DELAY_MS=400
TTS_TIMEOUT_SECONDS=120
```

- `HISTORY_MAX_LENGTH`：保留的用户/助手对话轮数。
- `MAX_RESPONSE_CHARS`：单次送入 TTS 的最大字符数。
- `REQUEST_TIMEOUT_SECONDS`：API 请求超时。
- `ABORT_DELAY_MS`：重启原生服务后，发起 API 请求前的等待时间。
- `TTS_TIMEOUT_SECONDS`：原生 TTS 调用超时。

## 额外请求头

```ini
HEADER_1=x-api-version: 2025-01-01
HEADER_2=another-header: value
```

## 检查配置

```sh
cd /data/open-xiaoai
./standalone-ai --check-config
```

输出只显示设备、模型和 Key 是否已配置，不显示完整 API Key。