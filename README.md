# LX06 Standalone AI

在小米小爱音箱 Pro（LX06）本机运行的独立中文 AI 客户端。它复用音箱原生的唤醒、ASR 和 TTS，在设备本机通过 OpenAI-compatible Chat Completions API 调用远程大模型，不需要电脑、NAS 或服务器常驻。

> 项目定位：真实 LX06 设备上的实验性开源项目，不是小米官方软件，也不是离线大模型。

## 功能

- ARMv7 Linux 原生 Rust 程序，直接运行在 LX06 `/data` 可写分区
- 复用原生小爱唤醒和语音识别
- 使用 `ubus call mibrain text_to_speech` 播放大模型回答
- 兼容 OpenAI-compatible API，可配置 Mimo、GPT、DeepSeek 以及其他兼容服务
- 多轮上下文
- 可自定义 System Prompt
- 混合路由：设备控制交给原生小爱，普通问答交给大模型
- API 超时、网络错误、认证错误和 JSON 错误时回退到原生小爱
- 余额不足、额度耗尽提示
- 配置文件权限 `600`，API Key 不进入源码、Git 或构建产物
- `/data/init.sh` 自启动
- GitHub Actions 自动构建 ARMv7 Artifact

## 已验证环境

| 项目 | 已验证值 |
|---|---|
| 设备 | Xiaomi 小爱音箱 Pro（LX06） |
| 固件 | `1.94.13 patched` |
| 架构 | ARMv7 hard-float |
| ASR 输入 | `/tmp/log/messages` 中的 `speech_recognizer.asr=...` |
| TTS | `mibrain.text_to_speech` |
| API 格式 | OpenAI-compatible `chat/completions` |

其他固件、其他型号和未打补丁设备不保证兼容。

## 工作原理

```text
小爱同学
  ↓
LX06 原生唤醒与 ASR
  ↓
/tmp/log/messages
  ↓
standalone-ai
  ├─ 设备控制/播放/音量/闹钟 → 原生小爱
  └─ 普通问答/解释/聊天 → OpenAI-compatible API
                              ↓
                         大模型回答
                              ↓
                    mibrain.text_to_speech
                              ↓
                         音箱播放
```

电脑只负责构建、上传和维护；程序运行期间不依赖电脑。

## 快速安装

### 方式 A：使用 GitHub Actions Artifact

1. 打开本仓库的 Actions 页面。
2. 运行 `Build LX06 Standalone AI`。
3. 下载成功运行产生的 `lx06-standalone-ai` Artifact。
4. 解压得到：

```text
standalone-ai
install-standalone-ai.sh
ai.conf.example
```

5. 通过 SSH 上传到 LX06 的同一目录：

```sh
ssh -o HostKeyAlgorithms=+ssh-rsa root@你的音箱IP
mkdir -p /data/open-xiaoai
```

在电脑端上传：

```sh
scp -O -o HostKeyAlgorithms=+ssh-rsa standalone-ai install-standalone-ai.sh root@你的音箱IP:/data/open-xiaoai/
```

如果设备的 SSH 不支持 SFTP，`scp -O` 是必要的；也可以使用 `dd` 管道上传。

在 LX06 上执行：

```sh
cd /data/open-xiaoai
chmod 755 standalone-ai install-standalone-ai.sh
sh install-standalone-ai.sh
```

安装脚本会备份已有二进制、配置、启动脚本和 `/data/init.sh`，不会修改 boot、system 或其他只读固件分区。

### 方式 B：从源码构建

需要 Rust、Docker 和 `cross`：

```sh
cross build --release --target armv7-unknown-linux-gnueabihf --bin standalone-ai
```

如果使用本仓库的精简源码布局：

```sh
cross build --release --target armv7-unknown-linux-gnueabihf --bin standalone-ai
```

产物位于：

```text
target/armv7-unknown-linux-gnueabihf/release/standalone-ai
```

## API 配置

配置文件：

```text
/data/open-xiaoai/ai.conf
```

首次安装后编辑示例：

```sh
cd /data/open-xiaoai
cp ai.conf ai.conf.bak
vi ai.conf
```

至少配置以下三项：

```ini
BASE_URL=https://api.xiaomimimo.com/v1
API_KEY=你的APIKey
MODEL=mimo-v2.6-flash
```

程序会自动请求：

```text
BASE_URL/chat/completions
```

支持的不是只有 GPT。只要服务兼容 OpenAI Chat Completions 格式，就可以填写对应的 URL 和模型名称。

配置完成后：

```sh
chmod 600 /data/open-xiaoai/ai.conf
sh /data/open-xiaoai/ai-start.sh
/data/open-xiaoai/standalone-ai --check-config
```

`--check-config` 不会输出 API Key。不要把真实 Key 提交到 GitHub、Issue、日志或截图中。

## Prompt 配置

Prompt 在 `ai.conf` 的 `SYSTEM_PROMPT` 中，可直接修改，不需要重新编译：

```ini
SYSTEM_PROMPT=你的System Prompt
```

Prompt 必须保持为一行；如果需要换行，使用字面量 `\n`。修改后重启：

```sh
sh /data/open-xiaoai/ai-start.sh
```

## 常用维护命令

```sh
# 查看配置概要
cd /data/open-xiaoai
./standalone-ai --check-config

# 查看程序是否运行
pidof standalone-ai

# 查看最近日志
tail -n 30 /tmp/open-xiaoai-ai.log

# 实时查看日志
tail -f /tmp/open-xiaoai-ai.log

# 查看路由、API 和 TTS 状态
grep -E 'route:|API error|API HTTP|fallback|timing:|tts:' /tmp/open-xiaoai-ai.log | tail -40
```

日志中的含义：

- `route: LLM`：本次调用了大模型；
- `route: native XiaoAI`：本次交给原生小爱；
- `API error` / `native fallback`：大模型失败后降级到原生小爱；
- `tts: accepted`：TTS 接口接受请求，不代表音频已经完整播放结束。

## 安全与限制

- 刷入 patched 固件、修改 `/data/init.sh` 和安装程序都有风险，请先备份。
- 不要在未经确认的情况下刷错型号或版本的固件。
- 不要把 API Key 写入源码、Shell 历史、GitHub Actions 日志或公开截图。
- 项目依赖小米原生 ASR/TTS 和网络，API 不可用时只能降级到原生小爱。
- 路由规则是本地关键词规则，不是完整意图识别；复杂中文表达可能需要补充规则。
- API 失败冷却期间，普通问题会直接交给原生小爱。
- 目前不保证所有 LX06 固件版本、OH2P 或其他型号兼容。
- 项目不会提取或复制小米云端官方 System Prompt；仓库中的 Prompt 均为自定义内容。

## 文档

- [安装教程](docs/INSTALL.md)
- [配置说明](docs/CONFIGURATION.md)
- [架构说明](docs/ARCHITECTURE.md)
- [故障排查](docs/TROUBLESHOOTING.md)
- [安装与运行说明](README-runtime-notes.md)
- [配置示例](ai.conf.example)
- [安装脚本](install-standalone-ai.sh)
- [构建工作流](.github/workflows/build.yml)

## 开源许可

本项目代码基于 MIT License 发布。请同时阅读仓库中的 `agreement.md`。

本项目与小米及相关 API 服务提供商没有隶属、合作或官方授权关系。小米、小爱、MiMo 等名称和商标归其各自权利人所有。
