# 配置说明

## API

```ini
BASE_URL=https://api.xiaomimimo.com/v1
API_KEY=CHANGE_ME
MODEL=mimo-v2.6-flash
```

`BASE_URL` 可以是任何兼容 OpenAI Chat Completions 的服务。程序自动追加 `/chat/completions`。

## Prompt

`SYSTEM_PROMPT` 必须是一行。换行使用 `\n`。Prompt 是用户自定义内容，不是小米官方 Prompt。

## 性能

```ini
HISTORY_MAX_LENGTH=10
MAX_RESPONSE_CHARS=1200
REQUEST_TIMEOUT_SECONDS=25
ABORT_DELAY_MS=400
TTS_TIMEOUT_SECONDS=120
```

## 额外请求头

```ini
HEADER_1=x-api-version: 2025-01-01
```
