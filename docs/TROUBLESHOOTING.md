# 故障排查

## 程序没有运行

```sh
pidof standalone-ai
tail -n 50 /tmp/open-xiaoai-ai.log
sh /data/open-xiaoai/ai-start.sh
```

## 普通问题由原生小爱回答

检查是否处于 API 冷却：

```sh
grep -E 'route:|cooldown|API error|fallback' /tmp/open-xiaoai-ai.log | tail -50
```

## API 超时

在设备上测试域名和 HTTPS：

```sh
nslookup api.example.com 2>/dev/null
curl -I --connect-timeout 10 https://api.example.com/v1
```

LX06 的网络环境可能无法访问某些服务。

## 看到 TTS failed 但设备实际播放

确认使用的是包含 `Stdio::piped()` 修复的版本，并查看：

```sh
grep -E 'tts:|mibrain text_to_speech' /tmp/open-xiaoai-ai.log | tail -20
```

## 判断来源

- `route: LLM`：大模型；
- `route: native XiaoAI`：原生小爱；
- `native fallback`：大模型失败后原生小爱兜底。
