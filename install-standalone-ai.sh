#!/bin/sh
set -eu
BASE_DIR=/data/open-xiaoai
BIN="$BASE_DIR/standalone-ai"
CONF="$BASE_DIR/ai.conf"
START="$BASE_DIR/ai-start.sh"
INIT=/data/init.sh
SCRIPT_DIR=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
SOURCE_BIN="$SCRIPT_DIR/standalone-ai"

backup_if_exists() {
  target=$1
  if [ -e "$target" ]; then
    backup="$target.bak"
    if [ -e "$backup" ]; then
      backup="$target.bak.$$"
    fi
    cp -p "$target" "$backup"
    echo "Backed up $target to $backup"
  fi
}

mkdir -p "$BASE_DIR"
if [ ! -f "$SOURCE_BIN" ]; then
  echo "standalone-ai binary not found next to installer: $SOURCE_BIN" >&2
  exit 1
fi
backup_if_exists "$BIN"
cp "$SOURCE_BIN" "$BIN.new"
chmod 755 "$BIN.new"
mv -f "$BIN.new" "$BIN"

if [ ! -f "$CONF" ]; then
  cat > "$CONF" <<'CFG'
BASE_URL=https://api.openai.com/v1
API_KEY=CHANGE_ME
MODEL=gpt-4.1-mini
SYSTEM_PROMPT=你是运行在小米小爱音箱 Pro 上的独立中文 AI 助手，性格亲切、聪明、自然，带一点轻松的幽默感，但不要油腻或刻意卖萌。你的回答会由音箱直接朗读，所以请使用自然流畅的口语表达，不要使用Markdown、表格、代码块、网址、Emoji或复杂排版。根据问题复杂度调整回答长度：简单问题直接回答，解释、分析、教程、故事、比较和技术问题可以充分展开，但不要重复、空泛或无意义地延长。你要有自己的判断，遇到不确定的内容要诚实说明，不要编造事实，也不要动不动使用“作为一个AI语言模型”之类生硬的开场。你是独立大模型回答，不是原生小爱；如果用户询问你的身份，请如实说明这一点，但不要主动介绍后台实现。设备控制、播放音乐、设置闹钟等操作由原生小爱负责处理；你可以解释操作方法、分析原因或提供建议，但不要声称自己已经执行了设备操作。对于天气、新闻、股价、实时交通等时效性问题，如果无法确认最新信息，就自然地说明目前无法可靠确认，不要编造，也不要机械地说“我没有联网搜索工具”。与用户交流时要像一个可靠、耐心、有个性的语音助手，先直接回答，再补充必要解释；用户需要详细内容时认真讲清楚，用户只想要结论时不要展开过多。
HISTORY_MAX_LENGTH=10
MAX_RESPONSE_CHARS=1200
REQUEST_TIMEOUT_SECONDS=25
ABORT_DELAY_MS=400
TTS_TIMEOUT_SECONDS=120
CFG
  chmod 600 "$CONF"
  echo "Created $CONF"
fi

backup_if_exists "$START"
cat > "$START" <<'SH'
#!/bin/sh
BASE_DIR=/data/open-xiaoai
BIN="$BASE_DIR/standalone-ai"
LOG=/tmp/open-xiaoai-ai.log
[ -x "$BIN" ] || exit 0

if pidof standalone-ai >/dev/null 2>&1; then
  exit 0
fi
"$BIN" >> "$LOG" 2>&1 &
SH
chmod 755 "$START"

if [ ! -f "$INIT" ]; then
  printf '%s\n' '#!/bin/sh' > "$INIT"
  chmod 755 "$INIT"
else
  backup_if_exists "$INIT"
fi
if ! grep -Fq "$START" "$INIT" 2>/dev/null; then
  printf '\n# open-xiaoai standalone AI\n[ -x "%s" ] && "%s"\n' "$START" "$START" >> "$INIT"
fi

killall standalone-ai 2>/dev/null || true
sleep 1
"$START"

echo "Standalone AI installed and started."
echo "Edit: $CONF"
