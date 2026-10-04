# Xiaomi XiaoAI 安装教程

## 当前支持建议

当前推荐先安装到已完成实机验证的 LX06。

OH2P 目前只有实验性平台适配和 AArch64 构建流程。没有真实 OH2P 设备、SSH 和日志时，不建议直接部署，也不要仅凭固件静态分析判断兼容。

## 前提

### LX06

- Xiaomi 小爱音箱 Pro LX06；
- 与设备匹配的 patched 固件；
- 已能通过 SSH 以 root 登录；
- 设备可以访问所选 API；
- 设备架构为 ARMv7 hard-float。

### OH2P

- 小米智能音箱 Pro OH2P；
- 已确认设备架构为 AArch64；
- 已能通过 SSH 以 root 登录；
- 能够获取 ASR、TTS 和启动日志；
- 使用 OH2P 专用 AArch64 Artifact。

## 1. 检查设备

在音箱 SSH 终端执行：

```sh
uname -m
mount
ls -ld /data
```

LX06 预期架构通常为：

```text
armv7l
```

OH2P 预期架构为：

```text
aarch64
```

如果架构和 Artifact 不匹配，不要继续安装。

项目只应写入 `/data` 可写区域，不要把程序写入 boot、system 或 vendor 分区。

## 2. 下载匹配的 Artifact

在 GitHub Actions 中选择：

```text
Build Xiaomi XiaoAI Standalone AI
```

LX06 下载：

```text
lx06-standalone-ai
```

OH2P 下载：

```text
oh2p-standalone-ai-experimental
```

OH2P Artifact 目前只是实验性构建包，不代表 OH2P 已经完成兼容性验收。

## 3. 上传文件

解压 Artifact 后，在电脑执行：

```sh
scp -O -o HostKeyAlgorithms=+ssh-rsa \
  standalone-ai install-standalone-ai.sh \
  root@设备IP:/data/open-xiaoai/
```

设备不支持 `scp` 的 SFTP 子系统时，必须加 `-O`。也可以使用其他安全的文件传输方式。

## 4. 安装

通过 SSH 登录音箱：

```sh
ssh -o HostKeyAlgorithms=+ssh-rsa root@设备IP
cd /data/open-xiaoai
chmod 755 standalone-ai install-standalone-ai.sh
sh install-standalone-ai.sh
```

安装脚本会尽量备份已有二进制、配置、启动脚本和 `/data/init.sh`，然后启动程序。

## 5. 设备配置

编辑配置：

```sh
vi /data/open-xiaoai/ai.conf
```

LX06：

```ini
DEVICE_MODEL=LX06
```

OH2P：

```ini
DEVICE_MODEL=OH2P
```

如果已经通过实机日志确认了其他 ASR 路径，可以增加：

```ini
ASR_LOG_PATHS=/tmp/mico_aivs_lab/instruction.log,/tmp/log/messages
```

## 6. API 配置

```ini
BASE_URL=https://api.xiaomimimo.com/v1
API_KEY=你的Key
MODEL=mimo-v2.6-flash
```

保存后执行：

```sh
chmod 600 /data/open-xiaoai/ai.conf
/data/open-xiaoai/standalone-ai --check-config
sh /data/open-xiaoai/ai-start.sh
```

确认输出中的设备型号和模型正确，并且：

```text
api_key_configured=true
```

## 7. 验证

查看日志：

```sh
tail -f /tmp/open-xiaoai-ai.log
```

普通问题应看到：

```text
route: LLM
```

设备控制，例如“打开卧室灯”，应看到：

```text
route: native XiaoAI
```

大模型回答成功时通常还会看到：

```text
tts: accepted
```

## 8. OH2P 额外验证

OH2P 不要只验证程序能启动，还需要逐项确认：

1. 说话后程序是否能从正确日志读取 ASR；
2. API 回答是否能通过原生 TTS 播放；
3. 原生设备控制是否仍然正常；
4. API 失败时是否能正确 fallback；
5. 重启后 `/data/init.sh` 是否能自动启动；
6. 连续运行数小时后是否稳定。

在这些项目确认前，OH2P 应始终标记为实验性支持。

## 9. 回滚

查看备份：

```sh
ls -lt /data/open-xiaoai/*bak* /data/init.sh.bak* 2>/dev/null
```

根据实际备份文件恢复，不要盲目覆盖。必要时先停止程序：

```sh
killall standalone-ai 2>/dev/null || true
```

不要删除不确定用途的系统文件。