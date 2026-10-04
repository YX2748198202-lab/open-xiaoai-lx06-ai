# XiaoAI Remix

在小米小爱音箱系列本机运行的独立中文 AI 客户端。

项目复用音箱原生的唤醒、ASR 和 TTS，在设备本机通过 OpenAI-compatible Chat Completions API 调用远程大模型，不需要电脑、NAS 或服务器常驻。

当前支持状态：

- **Xiaomi 小爱音箱 Pro（LX06）**：已完成真实设备验证，可以正常运行。
- **小米智能音箱 Pro（OH2P）**：已加入平台适配层和 AArch64 构建流程，目前仍需真实设备验证，不能视为完整支持。

> 本项目是第三方实验性开源项目，不是小米官方软件，也不是离线大模型。请不要把 LX06 的 ARMv7 二进制安装到 OH2P，也不要把 OH2P 的 AArch64 二进制安装到 LX06。

## 功能

- 复用原生小爱唤醒和语音识别
- 使用原生 `mibrain` TTS 播放大模型回答
- 兼容 OpenAI-compatible API，可配置 MiMo、GPT、DeepSeek 以及其他兼容服务
- 多轮上下文和自定义 System Prompt
- 混合路由：设备控制交给原生小爱，普通问答交给大模型
- API 超时、网络错误、认证错误和 JSON 错误时回退到原生小爱
- 余额不足、额度耗尽语音提示
- 配置文件权限 `600`，API Key 不进入源码、Git 或构建产物
- `/data/init.sh` 自启动
- 按设备架构生成独立 Artifact
- LX06 使用 ARMv7；OH2P 使用 AArch64 构建目标

## 设备支持状态

| 设备 | 型号 | 状态 | 架构 | 当前结论 |
|---|---|---|---|---|
| Xiaomi 小爱音箱 Pro | LX06 | 已实机验证 | ARMv7 hard-float | 固件 `1.94.13 patched` 下，ASR、TTS、路由、fallback 已验证 |
| 小米智能音箱 Pro | OH2P | 实验性适配 | 预计 AArch64 | 已分析 `OH2P_1.58.6_patched` 固件并加入适配框架，尚未完成实机验证 |

OH2P 的静态固件分析只能说明固件中存在 AIVS/MiBrain 相关组件，不能证明运行时 ASR 路径、TTS 参数、自启动和 fallback 与 LX06 完全一致。

## 工作原理

```text
原生唤醒
  ↓
设备原生 ASR
  ↓
平台适配层读取 ASR 事件
  ↓
standalone-ai
  ├─ 设备控制 / 播放 / 音量 / 闹钟 → 原生小爱
  └─ 普通问答 / 解释 / 聊天 → OpenAI-compatible API
                                      ↓
                                 大模型回答
                                      ↓
                              原生 mibrain TTS
                                      ↓
                                  音箱播放
```

API 请求、Prompt、多轮历史、路由和 fallback 逻辑由各设备共用；设备架构、ASR 日志路径、原生服务和 TTS 接口通过 `src/platform.rs` 集中管理。

## 发行版

每个正式版本会提供 GitHub Release：

- `xiaoai-remix-lx06-vX.Y.Z.tar.gz`：LX06 ARMv7，已验证设备使用；
- `xiaoai-remix-oh2p-vX.Y.Z-experimental.tar.gz`：OH2P AArch64，实验性构建，仅供真实设备验证。

发行版由推送 `vX.Y.Z` 标签自动生成。普通分支构建产生的是 Actions Artifact，不是正式发行版。

目前 LX06 和 OH2P 使用同一套源码，但二进制架构不同，下载时必须选择匹配设备的压缩包。

## 快速安装

### LX06

LX06 使用 ARMv7 Artifact：

```text
lx06-standalone-ai
```

从 GitHub Actions 下载后解压得到：

```text
standalone-ai
install-standalone-ai.sh
ai.conf.example
```

在电脑端上传：

```sh
scp -O -o HostKeyAlgorithms=+ssh-rsa \
  standalone-ai install-standalone-ai.sh \
  root@你的音箱IP:/data/open-xiaoai/
```

在 LX06 上执行：

```sh
ssh -o HostKeyAlgorithms=+ssh-rsa root@你的音箱IP
cd /data/open-xiaoai
chmod 755 standalone-ai install-standalone-ai.sh
sh install-standalone-ai.sh
```

安装脚本会备份已有二进制、配置、启动脚本和 `/data/init.sh`，只使用 `/data` 可写区域，不会主动修改 boot、system 或 vendor 分区。

### OH2P

OH2P 使用独立的 AArch64 实验性 Artifact：

```text
oh2p-standalone-ai-experimental
```

但当前不建议在没有真实设备和 SSH 的情况下直接安装。正式部署前至少需要确认：

- `uname -m` 是否为 `aarch64`
- 实际 ASR 日志路径和格式
- `mibrain.text_to_speech` 参数及播放结果
- 原生服务重启方式
- `mibrain.ai_service` fallback
- `/data/init.sh` 自启动
- 长时间运行稳定性

不能把 LX06 的 Artifact 安装到 OH2P，也不能把 OH2P Artifact 安装到 LX06。

## 从源码构建

需要 Rust、Docker 和 `cross`。

LX06：

```sh
cross build --release \
  --target armv7-unknown-linux-gnueabihf \
  --bin standalone-ai
```

OH2P：

```sh
cross build --release \
  --target aarch64-unknown-linux-gnu \
  --bin standalone-ai
```

构建前建议运行：

```sh
cargo fmt --all -- --check
cargo test --all
```

## 设备配置

配置文件：

```text
/data/open-xiaoai/ai.conf
```

选择设备档案：

```ini
DEVICE_MODEL=LX06
```

可选值：

```text
LX06
OH2P
```

`OH2P` 只代表启用实验性平台适配，不代表已经完成硬件兼容性验收。

如果在真实设备上确认了不同的 ASR 日志路径，可以临时覆盖：

```ini
ASR_LOG_PATHS=/tmp/mico_aivs_lab/instruction.log,/tmp/log/messages
```

不配置时，程序使用对应设备档案中的候选路径。

## API 配置

至少配置：

```ini
BASE_URL=https://api.xiaomimimo.com/v1
API_KEY=你的APIKey
MODEL=mimo-v2.6-flash
```

程序会自动请求：

```text
BASE_URL/chat/completions
```

只要服务兼容 OpenAI Chat Completions 格式，就可以使用对应的 URL 和模型名称。

配置完成后：

```sh
chmod 600 /data/open-xiaoai/ai.conf
sh /data/open-xiaoai/ai-start.sh
/data/open-xiaoai/standalone-ai --check-config
```

`--check-config` 不会输出完整 API Key。不要把真实 Key 提交到 GitHub、Issue、日志或截图中。

## Prompt 配置

Prompt 位于 `ai.conf` 的 `SYSTEM_PROMPT`，可以直接修改，不需要重新编译：

```ini
SYSTEM_PROMPT=你的System Prompt
```

Prompt 必须保持为一行；需要换行时使用字面量 `\\n`。修改后重启：

```sh
sh /data/open-xiaoai/ai-start.sh
```

仓库中的 Prompt 是自定义 Prompt，不是小米云端官方 Prompt。

## 常用维护命令

```sh
cd /data/open-xiaoai

# 查看配置概要
./standalone-ai --check-config

# 查看程序是否运行
pidof standalone-ai

# 查看最近日志
tail -n 30 /tmp/open-xiaoai-ai.log

# 实时查看日志
tail -f /tmp/open-xiaoai-ai.log

# 查看路由、API 和 TTS 状态
grep -E 'route:|API error|fallback|timing:|tts:' \
  /tmp/open-xiaoai-ai.log | tail -40
```

日志含义：

- `route: LLM`：本次调用了大模型；
- `route: native XiaoAI`：本次交给原生小爱；
- `API error` / `native fallback`：大模型失败后降级到原生小爱；
- `tts: accepted`：TTS 接口接受请求，不代表音频已经完整播放结束。

## 安全与限制

- 刷入 patched 固件、修改 `/data/init.sh` 和安装程序都有风险，请先备份。
- 不要把错误型号或错误架构的二进制安装到设备上。
- 不要在未经确认的情况下刷错型号或版本的固件。
- 不要把 API Key 写入源码、Shell 历史、GitHub Actions 日志或公开截图。
- 项目依赖原生 ASR、TTS 和网络，API 不可用时才会降级到原生小爱。
- 路由规则是本地关键词规则，不是完整意图识别；复杂中文表达可能需要补充规则。
- API 失败冷却期间，普通问题会直接交给原生小爱。
- LX06 当前只对已验证的固件和运行环境负责。
- OH2P 仍属于实验性适配，静态固件分析不能替代实机验收。
- 项目不会提取或复制小米云端官方 System Prompt。

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
