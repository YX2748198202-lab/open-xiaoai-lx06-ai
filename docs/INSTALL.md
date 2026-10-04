# LX06 安装教程

## 前提

- Xiaomi 小爱音箱 Pro LX06
- 已刷入与设备匹配的 patched 固件
- 已能通过 SSH 以 root 登录
- 设备能访问你选择的 API

## 1. 检查设备

```sh
uname -m
mount
ls -ld /data
```

本项目目标环境是 ARMv7、根文件系统只读、`/data` 可写。不要把程序写入 boot、system 或 vendor 分区。

## 2. 上传文件

从 Actions Artifact 解压出三个文件，在电脑执行：

```sh
scp -O -o HostKeyAlgorithms=+ssh-rsa standalone-ai install-standalone-ai.sh root@设备IP:/data/open-xiaoai/
```

设备不支持 `scp` 的 SFTP 子系统时必须加 `-O`。

## 3. 安装

```sh
ssh -o HostKeyAlgorithms=+ssh-rsa root@设备IP
cd /data/open-xiaoai
chmod 755 standalone-ai install-standalone-ai.sh
sh install-standalone-ai.sh
```

## 4. 配置

```sh
vi /data/open-xiaoai/ai.conf
```

配置：

```ini
BASE_URL=https://api.xiaomimimo.com/v1
API_KEY=你的Key
MODEL=mimo-v2.6-flash
```

保存后：

```sh
chmod 600 /data/open-xiaoai/ai.conf
sh /data/open-xiaoai/ai-start.sh
/data/open-xiaoai/standalone-ai --check-config
```

## 5. 验证

```sh
tail -f /tmp/open-xiaoai-ai.log
```

说一个普通问题，日志应显示 `route: LLM`。说“打开卧室灯”，应显示 `route: native XiaoAI`。

## 6. 回滚

查看备份：

```sh
ls -lt /data/open-xiaoai/*bak* /data/init.sh.bak* 2>/dev/null
```

根据实际备份文件恢复，不要盲目覆盖。
