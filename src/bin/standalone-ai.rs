use open_xiaoai::base::AppError;
use open_xiaoai::platform::DeviceModel;
use open_xiaoai::services::monitor::file::{FileMonitor, FileMonitorEvent};
use serde::Deserialize;
use serde_json::{json, Value};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::{sleep, timeout};

const CONFIG_PATH: &str = "/data/open-xiaoai/ai.conf";
const BALANCE_NOTICE_INTERVAL: Duration = Duration::from_secs(1800);
const BALANCE_NOTICE: &str =
    "大模型服务提示余额不足，已暂时切回原小爱。请充值后再试，现在请再说一次问题。";
const QUOTA_NOTICE: &str =
    "大模型服务提示可用额度已耗尽，已暂时切回原小爱。请检查套餐或用量，现在请再说一次问题。";
const API_FAILURES_BEFORE_FALLBACK: u8 = 1;
const NATIVE_FALLBACK_COOLDOWN: Duration = Duration::from_secs(300);

fn default_system_prompt() -> String {
    r#"你是运行在小米小爱音箱 Pro 上的独立中文 AI 助手，性格亲切、聪明、自然，带一点轻松的幽默感，但不要油腻或刻意卖萌。你的回答会由音箱直接朗读，所以请使用自然流畅的口语表达，不要使用Markdown、表格、代码块、网址、Emoji或复杂排版。根据问题复杂度调整回答长度：简单问题直接回答，解释、分析、教程、故事、比较和技术问题可以充分展开，但不要重复、空泛或无意义地延长。你要有自己的判断，遇到不确定的内容要诚实说明，不要编造事实，也不要动不动使用“作为一个AI语言模型”之类生硬的开场。你是独立大模型回答，不是原生小爱；如果用户询问你的身份，请如实说明这一点，但不要主动介绍后台实现。设备控制、播放音乐、设置闹钟等操作由原生小爱负责处理；你可以解释操作方法、分析原因或提供建议，但不要声称自己已经执行了设备操作。对于天气、新闻、股价、实时交通等时效性问题，如果无法确认最新信息，就自然地说明目前无法可靠确认，不要编造，也不要机械地说“我没有联网搜索工具”。与用户交流时要像一个可靠、耐心、有个性的语音助手，先直接回答，再补充必要解释；用户需要详细内容时认真讲清楚，用户只想要结论时不要展开过多。"#.to_string()
}

#[derive(Clone)]
struct Config {
    device_model: DeviceModel,
    base_url: String,
    api_key: String,
    model: String,
    system_prompt: String,
    history_max_length: usize,
    max_response_chars: usize,
    request_timeout_seconds: u64,
    abort_delay_ms: u64,
    tts_timeout_seconds: u64,
    request_headers: Vec<(String, String)>,
    asr_log_paths: Option<Vec<String>>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device_model: DeviceModel::default(),
            base_url: "https://api.openai.com/v1".to_string(),
            api_key: String::new(),
            model: "gpt-4.1-mini".to_string(),
            system_prompt: default_system_prompt(),
            history_max_length: 10,
            max_response_chars: 1200,
            request_timeout_seconds: 25,
            abort_delay_ms: 400,
            tts_timeout_seconds: 120,
            request_headers: Vec::new(),
            asr_log_paths: None,
        }
    }
}

impl Config {
    async fn load(path: &str) -> Result<Self, AppError> {
        let content = fs::read_to_string(path).await?;
        let mut config = Self::default();

        for raw_line in content.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim();
            let value = value.trim().trim_matches('"');

            match key {
                "DEVICE_MODEL" => {
                    config.device_model =
                        DeviceModel::parse(value).map_err(|err| -> AppError { err.into() })?;
                }
                "BASE_URL" => config.base_url = value.to_string(),
                "API_KEY" => config.api_key = value.to_string(),
                "MODEL" => config.model = value.to_string(),
                "SYSTEM_PROMPT" => config.system_prompt = value.replace("\\n", "\n"),
                "HISTORY_MAX_LENGTH" => config.history_max_length = value.parse().unwrap_or(10),
                "MAX_RESPONSE_CHARS" => config.max_response_chars = value.parse().unwrap_or(1200),
                "REQUEST_TIMEOUT_SECONDS" => {
                    config.request_timeout_seconds = value.parse().unwrap_or(25)
                }
                "ABORT_DELAY_MS" => config.abort_delay_ms = value.parse().unwrap_or(400),
                "TTS_TIMEOUT_SECONDS" => config.tts_timeout_seconds = value.parse().unwrap_or(120),
                "ASR_LOG_PATHS" => {
                    let paths = value
                        .split(',')
                        .map(str::trim)
                        .filter(|path| !path.is_empty())
                        .map(ToOwned::to_owned)
                        .collect::<Vec<_>>();
                    if !paths.is_empty() {
                        config.asr_log_paths = Some(paths);
                    }
                }
                _ if key.starts_with("HEADER_") => {
                    if let Some((name, header_value)) = value.split_once(':') {
                        config
                            .request_headers
                            .push((name.trim().to_string(), header_value.trim().to_string()));
                    }
                }
                _ => {}
            }
        }

        if config.base_url.is_empty() {
            return Err("BASE_URL 不能为空".into());
        }
        if config.api_key.is_empty() {
            return Err("API_KEY 不能为空".into());
        }
        if config.model.is_empty() {
            return Err("MODEL 不能为空".into());
        }
        if config.history_max_length > 50 {
            config.history_max_length = 50;
        }
        if config.max_response_chars < 50 {
            config.max_response_chars = 50;
        }
        if config.max_response_chars > 4000 {
            config.max_response_chars = 4000;
        }

        Ok(config)
    }

    fn monitor_paths(&self) -> Vec<String> {
        self.asr_log_paths.clone().unwrap_or_else(|| {
            self.device_model
                .monitor_paths()
                .iter()
                .map(|path| (*path).to_string())
                .collect()
        })
    }

    fn endpoint(&self) -> String {
        let base = self.base_url.trim_end_matches('/');
        if base.ends_with("/chat/completions") {
            base.to_string()
        } else {
            format!("{base}/chat/completions")
        }
    }
}

#[derive(Clone, Debug, Deserialize)]
struct ChatResponse {
    choices: Option<Vec<ChatChoice>>,
    error: Option<Value>,
}

#[derive(Clone, Debug, Deserialize)]
struct ChatChoice {
    message: Option<ChatMessageOwned>,
    text: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ChatMessageOwned {
    content: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ApiFailureKind {
    Balance,
    Quota,
    Authentication,
    RateLimitOrQuota,
    Other,
}
impl ApiFailureKind {
    fn label(self) -> &'static str {
        match self {
            Self::Balance => "balance_insufficient",
            Self::Quota => "quota_exhausted",
            Self::Authentication => "authentication_or_access",
            Self::RateLimitOrQuota => "rate_limit_or_quota_unknown",
            Self::Other => "api_error",
        }
    }
}
#[derive(Debug)]
struct ApiFailure {
    status: u16,
    kind: ApiFailureKind,
}
impl std::fmt::Display for ApiFailure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "API HTTP {} category={}", self.status, self.kind.label())
    }
}
impl std::error::Error for ApiFailure {}

fn classify_api_failure(status: u16, error: Option<&Value>) -> ApiFailure {
    // Inspect error fields only, never assistant content or arbitrary HTML.
    let mut hints = String::new();
    if let Some(err) = error {
        if let Some(text) = err.as_str() {
            hints.push_str(text);
        }
        for key in ["code", "type", "message"] {
            if let Some(text) = err.get(key).and_then(Value::as_str) {
                hints.push(' ');
                hints.push_str(text);
            }
        }
    }
    let hints = hints.to_lowercase();
    let kind = if status == 401 || status == 403 {
        ApiFailureKind::Authentication
    } else if status == 402
        || [
            "insufficient_balance",
            "insufficient balance",
            "balance_not_enough",
            "balance is insufficient",
            "balance is not enough",
            "余额不足",
            "账户欠费",
            "insufficient credits",
        ]
        .iter()
        .any(|x| hints.contains(x))
    {
        ApiFailureKind::Balance
    } else if [
        "insufficient_quota",
        "quota_exhausted",
        "quota exhausted",
        "额度耗尽",
        "额度已用完",
        "额度不足",
    ]
    .iter()
    .any(|x| hints.contains(x))
    {
        ApiFailureKind::Quota
    } else if status == 429 {
        ApiFailureKind::RateLimitOrQuota
    } else {
        ApiFailureKind::Other
    };
    ApiFailure { status, kind }
}

fn parse_api_http_response(body: &str, status: u16, max_chars: usize) -> Result<String, AppError> {
    if !(200..300).contains(&status) {
        let parsed: Option<Value> = serde_json::from_str(body).ok();
        let error = parsed
            .as_ref()
            .and_then(|v| v.get("error"))
            .filter(|e| !e.is_null());
        return Err(Box::new(classify_api_failure(status, error)));
    }
    parse_chat_response(body, max_chars)
}

fn claim_balance_notice(
    state: &mut State,
    kind: ApiFailureKind,
    now: Instant,
) -> Option<&'static str> {
    let notice = match kind {
        ApiFailureKind::Balance => BALANCE_NOTICE,
        ApiFailureKind::Quota => QUOTA_NOTICE,
        _ => return None,
    };
    if state
        .last_balance_notice
        .is_some_and(|at| now.saturating_duration_since(at) < BALANCE_NOTICE_INTERVAL)
    {
        return None;
    }
    state.last_balance_notice = Some(now);
    Some(notice)
}

#[derive(Clone, Debug)]
struct HistoryMessage {
    role: String,
    content: String,
}

#[derive(Default)]
struct State {
    busy: bool,
    history: Vec<HistoryMessage>,
    last_text: Option<(String, Instant)>,
    consecutive_api_failures: u8,
    native_fallback_until: Option<Instant>,
    generation: u64,
    last_balance_notice: Option<Instant>,
}

#[tokio::main]
async fn main() {
    println!("open-xiaoai standalone AI starting");

    let args: Vec<String> = std::env::args().skip(1).collect();
    let mut config_path = CONFIG_PATH.to_string();
    let mut ask = None;
    let mut check_config = false;
    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--config" if i + 1 < args.len() => {
                i += 1;
                config_path = args[i].clone();
            }
            "--ask" if i + 1 < args.len() => {
                i += 1;
                ask = Some(args[i].clone());
            }
            "--check-config" => check_config = true,
            _ => {
                eprintln!("usage: standalone-ai [--config PATH] [--check-config | --ask TEXT]");
                return;
            }
        }
        i += 1;
    }
    let config = match Config::load(&config_path).await {
        Ok(config) => Arc::new(config),
        Err(err) => {
            eprintln!("configuration error: {err}");
            return;
        }
    };

    if check_config {
        println!("config OK; device={}; model={}; api_key_configured={}; timeout={}s; abort_delay={}ms; max_chars={}",
            config.device_model.name(), config.model, api_key_configured(&config),
            config.request_timeout_seconds, config.abort_delay_ms, config.max_response_chars);
        return;
    }
    if let Some(text) = ask {
        if let Err(err) =
            handle_user_text(text, config, Arc::new(Mutex::new(State::default()))).await
        {
            eprintln!("test request failed: {err}");
            std::process::exit(1);
        }
        return;
    }
    println!("device: {}", config.device_model.name());
    if config.device_model == DeviceModel::Oh2p {
        println!("warning: OH2P profile is experimental; verify ASR, TTS, native fallback and startup on real hardware");
    }
    println!("API endpoint: {}", config.endpoint());
    println!("model: {}", config.model);
    println!("waiting for XiaoAI SpeechRecognizer events...");

    let state = Arc::new(Mutex::new(State::default()));
    let mut monitors = Vec::new();
    for path in config.monitor_paths() {
        let state_for_callback = Arc::clone(&state);
        let config_for_callback = Arc::clone(&config);
        let mut monitor = FileMonitor::new();
        monitor
            .start(&path, move |event| {
                let state = Arc::clone(&state_for_callback);
                let config = Arc::clone(&config_for_callback);
                async move {
                    if let FileMonitorEvent::NewLine(line) = event {
                        if let Some(text) = parse_recognition_line(&line) {
                            tokio::spawn(async move {
                                if let Err(err) = handle_user_text(text, config, state).await {
                                    eprintln!("standalone AI error: {err}");
                                }
                            });
                        }
                    }
                    Ok(())
                }
            })
            .await;
        println!("monitor: {path}");
        monitors.push(monitor);
    }

    loop {
        sleep(Duration::from_secs(60)).await;
    }
}

fn parse_recognition_line(line: &str) -> Option<String> {
    parse_json_recognition(line).or_else(|| parse_syslog_recognition(line))
}

fn parse_json_recognition(line: &str) -> Option<String> {
    let value: Value = serde_json::from_str(line).ok()?;
    if value.get("header")?.get("namespace")?.as_str()? != "SpeechRecognizer" {
        return None;
    }
    if value.get("header")?.get("name")?.as_str()? != "RecognizeResult" {
        return None;
    }

    let payload = value.get("payload")?;
    if !payload.get("is_final")?.as_bool()? {
        return None;
    }

    let text = payload
        .get("results")?
        .as_array()?
        .first()?
        .get("text")?
        .as_str()?
        .trim();

    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn parse_syslog_recognition(line: &str) -> Option<String> {
    let (prefix, rest) = line.split_once("speech_recognizer.asr=")?;
    if !prefix.contains("mico_aivs_lab") {
        return None;
    }
    let (text, flags) = rest.rsplit_once(", .final=")?;
    // nlp=true is an observed handoff marker, not the ASR is_final flag.
    if !flags.split(", ").any(|part| part.trim() == "nlp=true") {
        return None;
    }
    let text = text.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

fn api_key_configured(config: &Config) -> bool {
    !config.api_key.is_empty()
        && config.api_key != "CHANGE_ME"
        && !config.api_key.starts_with("sk-xxxx")
}

async fn handle_user_text(
    text: String,
    config: Arc<Config>,
    state: Arc<Mutex<State>>,
) -> Result<(), AppError> {
    let generation;
    {
        let mut locked = state.lock().await;
        if locked
            .last_text
            .as_ref()
            .is_some_and(|(t, at)| t == &text && at.elapsed() < Duration::from_secs(3))
        {
            return Ok(());
        }
        locked.last_text = Some((text.clone(), Instant::now()));
        locked.generation = locked.generation.wrapping_add(1);
        generation = locked.generation;
        if is_native_control_command(&text) || !api_key_configured(&config) {
            println!("route: native XiaoAI (device command/unconfigured API): {text}");
            return Ok(()); // Never replay a device command: native XiaoAI already owns it.
        }
        if locked
            .native_fallback_until
            .is_some_and(|until| until > Instant::now())
        {
            let remaining = locked
                .native_fallback_until
                .unwrap()
                .saturating_duration_since(Instant::now())
                .as_secs();
            println!("route: native XiaoAI (API cooldown; remaining={remaining}s): {text}");
            return Ok(());
        }
        if locked.busy {
            println!("route: native XiaoAI (busy; discard older AI answer): {text}");
            return Ok(());
        }
        locked.busy = true;
    }
    let started = Instant::now();
    let result = async {
        println!("route: LLM; model={}", config.model);
        println!("user: {text}");
        if let Err(err) = abort_xiaoai(&config.device_model).await {
            state.lock().await.native_fallback_until =
                Some(Instant::now() + NATIVE_FALLBACK_COOLDOWN);
            eprintln!("takeover failed: {err}; no AI reply or replay; native cooldown 300s");
            return Err(err);
        }
        sleep(Duration::from_millis(config.abort_delay_ms)).await;
        println!("timing: takeover_ms={}", started.elapsed().as_millis());
        if state.lock().await.generation != generation {
            return Ok(());
        }
        let api_started = Instant::now();
        let response =
            request_chat_completion(&text, Arc::clone(&config), Arc::clone(&state)).await;
        println!("timing: api_ms={}", api_started.elapsed().as_millis());
        if state.lock().await.generation != generation {
            println!("discard: newer native/user request; no stale TTS or fallback");
            return Ok(());
        }
        match response {
            Ok(answer) => {
                {
                    let mut locked = state.lock().await;
                    locked.consecutive_api_failures = 0;
                    locked.native_fallback_until = None;
                }
                println!("assistant: {answer}");
                let tts_started = Instant::now();
                speak_text(&answer, config.tts_timeout_seconds, &config.device_model).await?;
                println!(
                    "tts: accepted; tts_ms={}; total_ms={} (not playback duration)",
                    tts_started.elapsed().as_millis(),
                    started.elapsed().as_millis()
                );
                let mut locked = state.lock().await;
                locked.history.push(HistoryMessage {
                    role: "user".into(),
                    content: text,
                });
                locked.history.push(HistoryMessage {
                    role: "assistant".into(),
                    content: answer,
                });
                trim_history(&mut locked.history, config.history_max_length);
                Ok(())
            }
            Err(api_error) => {
                eprintln!("API error: {api_error}");
                let kind = api_error.downcast_ref::<ApiFailure>().map(|e| e.kind).unwrap_or(ApiFailureKind::Other);
                {
                    let mut locked = state.lock().await;
                    locked.consecutive_api_failures =
                        locked.consecutive_api_failures.saturating_add(1);
                    if locked.consecutive_api_failures >= API_FAILURES_BEFORE_FALLBACK {
                        locked.native_fallback_until =
                            Some(Instant::now() + NATIVE_FALLBACK_COOLDOWN);
                    }
                }
                println!("route: native fallback once; cooldown=300s; nlp_execute=0");
                // Question-only fallback. Do not resubmit an actionable smart-home operation.
                // Fast failures can occur before the restarted native cloud session is ready.
                let settle = Duration::from_secs(3).saturating_sub(started.elapsed());
                sleep(settle).await;
                if state.lock().await.generation != generation {
                    return Ok(());
                }
                let notice = {
                    let mut locked = state.lock().await;
                    claim_balance_notice(&mut locked, kind, Instant::now())
                };
                if let Some(notice) = notice {
                    // No immediate replay: native TTS could interrupt this warning.
                    println!("notice: {}; interval=1800s; native cooldown=300s", kind.label());
                    match speak_text(notice, 20, &config.device_model).await {
                        Ok(()) => {
                            println!("notice: accepted; next spoken question stays native during cooldown");
                            return Ok(());
                        }
                        Err(err) => eprintln!("notice TTS failed: {err}; attempting native fallback"),
                    }
                }
                if state.lock().await.generation != generation { return Ok(()); }
                ask_native_xiaoai(&text, &config.device_model).await?;
                println!(
                    "native fallback: accepted; total_ms={}",
                    started.elapsed().as_millis()
                );
                Ok(())
            }
        }
    }
    .await;
    state.lock().await.busy = false;
    result
}

fn is_native_control_command(text: &str) -> bool {
    let t = text.trim().trim_end_matches(['。', '！', '!', '？', '?']);
    let explanations = [
        "原理",
        "为什么",
        "怎么工作",
        "是什么意思",
        "如何",
        "怎么回事",
        "怎么",
        "教我",
        "解释",
        "介绍",
        "推荐",
        "区别",
        "什么是",
        "有什么好处",
        "有什么坏处",
    ];
    if explanations.iter().any(|w| t.contains(w)) {
        return false;
    }
    // Short media commands do not necessarily mention a device name.
    if [
        "暂停",
        "停止播放",
        "继续播放",
        "下一首",
        "上一首",
        "静音",
        "取消静音",
        "大声一点",
        "小声一点",
        "声音大一点",
        "声音小一点",
    ]
    .contains(&t)
    {
        return true;
    }
    if [
        "提醒我",
        "分钟后提醒",
        "小时后提醒",
        "播放",
        "放一首",
        "放首",
        "听歌",
        "定时",
        "倒计时",
    ]
    .iter()
    .any(|w| t.contains(w))
    {
        return true;
    }
    let devices = [
        "灯",
        "照明",
        "空调",
        "电视",
        "窗帘",
        "插座",
        "插排",
        "风扇",
        "扫地",
        "加湿器",
        "净化器",
        "热水器",
        "音量",
        "闹钟",
        "音乐",
        "设备",
        "门锁",
    ];
    let actions = [
        "打开",
        "关闭",
        "开一下",
        "关一下",
        "开灯",
        "关灯",
        "调高",
        "调低",
        "调暗",
        "调亮",
        "调到",
        "调成",
        "调为",
        "设为",
        "设成",
        "大一点",
        "小一点",
        "设置",
        "设个",
        "定个",
        "启动",
        "停止",
        "暂停",
        "回充",
        "取消",
        "删除",
        "拉开",
        "拉上",
        "关上",
    ];
    let have_device = devices.iter().any(|w| t.contains(w));
    have_device
        && (actions.iter().any(|w| t.contains(w))
            || t.starts_with(['开', '关'])
            || t.contains("开着")
            || t.contains("关着")
            || t.contains("状态")
            || t.contains("亮度")
            || t.contains("温度")
            || t.contains("把")
            || t.contains("帮我"))
}

async fn ask_native_xiaoai(text: &str, device: &DeviceModel) -> Result<(), AppError> {
    let request = json!({
        "tts": 1,
        "nlp": 1,
        "nlp_text": text,
        "nlp_execute": 0,
    });
    let child = Command::new("ubus")
        .arg("-t")
        .arg("25")
        .arg("call")
        .arg(device.ubus_object())
        .arg(device.ai_method())
        .arg(request.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let output = timeout(Duration::from_secs(25), child.wait_with_output()).await??;
    if !output.status.success() {
        return Err(format!(
            "mibrain ai_service exit status: {}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    check_ubus_response(&output.stdout, "ai_service")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{is_native_control_command, parse_syslog_recognition};

    #[test]
    fn routes_device_controls_to_native_xiaoai() {
        assert!(is_native_control_command("打开卧室灯光"));
        assert!(is_native_control_command("把客厅空调温度调高"));
        assert!(!is_native_control_command("开灯原理是什么"));
        assert!(!is_native_control_command("离子是什么意思"));
    }

    #[test]
    fn parses_only_nlp_syslog_asr_results() {
        assert_eq!(
            parse_syslog_recognition(
                "mico_aivs_lab: speech_recognizer.asr=测试独立AI, .final=false, nlp=true"
            ),
            Some("测试独立AI".to_string())
        );
        assert_eq!(
            parse_syslog_recognition("mico_aivs_lab: speech_recognizer.asr=中间结果, .final=false"),
            None
        );
    }
}

fn trim_history(history: &mut Vec<HistoryMessage>, max_messages: usize) {
    if max_messages == 0 {
        history.clear();
        return;
    }

    let keep = max_messages.saturating_mul(2);
    if history.len() > keep {
        let drop_count = history.len() - keep;
        history.drain(0..drop_count);
    }
}

async fn abort_xiaoai(device: &DeviceModel) -> Result<(), AppError> {
    let child = Command::new(device.native_service())
        .arg("restart")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let output = timeout(Duration::from_secs(10), child.wait_with_output()).await??;
    if !output.status.success() {
        return Err("native restart failed".into());
    }
    Ok(())
}

fn check_ubus_response(body: &[u8], method: &str) -> Result<(), AppError> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| format!("ubus {method}: invalid JSON"))?;
    match value.get("code").and_then(Value::as_i64) {
        Some(0) => Ok(()),
        code => Err(format!("ubus {method}: code={code:?}").into()),
    }
}

async fn speak_text(
    text: &str,
    timeout_seconds: u64,
    device: &DeviceModel,
) -> Result<(), AppError> {
    if text.trim().is_empty() {
        return Ok(());
    }
    let request = json!({
        "text": text.trim(),
        "caller": "standalone-ai",
        "play": 1,
    });
    let child = Command::new("ubus")
        .arg("-t")
        .arg(timeout_seconds.max(5).to_string())
        .arg("call")
        .arg(device.ubus_object())
        .arg(device.tts_method())
        .arg(request.to_string())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)
        .spawn()?;
    let output = timeout(
        Duration::from_secs(timeout_seconds.max(5)),
        child.wait_with_output(),
    )
    .await??;
    if !output.status.success() {
        return Err(format!(
            "mibrain text_to_speech exit status: {}; stderr={}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )
        .into());
    }
    check_ubus_response(&output.stdout, "text_to_speech")?;
    Ok(())
}

async fn request_chat_completion(
    text: &str,
    config: Arc<Config>,
    state: Arc<Mutex<State>>,
) -> Result<String, AppError> {
    let history = {
        let locked = state.lock().await;
        locked.history.clone()
    };

    let mut messages: Vec<Value> = Vec::with_capacity(history.len() + 2);
    if !config.system_prompt.trim().is_empty() {
        messages.push(json!({
            "role": "system",
            "content": config.system_prompt,
        }));
    }
    messages.push(json!({
        "role": "system",
        "content": format!("运行时提供的真实模型标识是 {}。你是独立大模型回答，不是原生小爱。不要虚构已经执行的设备操作；对于天气、新闻、股价等实时信息，无法可靠确认时要诚实说明。", config.model),
    }));
    for item in history.iter() {
        messages.push(json!({
            "role": item.role,
            "content": item.content,
        }));
    }
    messages.push(json!({
        "role": "user",
        "content": text,
    }));

    let request_body = json!({
        "model": config.model,
        "messages": messages,
        "stream": false,
    });

    // Pass credentials and body over stdin: no key in argv or temporary file.
    let mut curl_config = format!(
        "url = \"{}\"\nrequest = \"POST\"\nhttp1.1\nsilent\nshow-error\n",
        escape_curl_config(&config.endpoint())
    );
    curl_config.push_str(&format!(
        "header = \"Authorization: Bearer {}\"\n",
        escape_curl_config(&config.api_key)
    ));
    curl_config.push_str("header = \"Content-Type: application/json\"\n");
    for (name, value) in &config.request_headers {
        curl_config.push_str(&format!(
            "header = \"{}: {}\"\n",
            escape_curl_config(name),
            escape_curl_config(value)
        ));
    }
    curl_config.push_str(&format!(
        "data-binary = \"{}\"\n",
        escape_curl_config(&request_body.to_string())
    ));
    let seconds = config.request_timeout_seconds.clamp(5, 180);
    let run = async {
        let mut child = Command::new("curl")
            .arg("-q")
            .arg("--config")
            .arg("-")
            .arg("--connect-timeout")
            .arg(seconds.min(8).to_string())
            .arg("--max-time")
            .arg(seconds.to_string())
            .arg("--write-out")
            .arg("\nSTANDALONE_AI_HTTP_STATUS:%{http_code}")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut stdin = child.stdin.take().ok_or("curl stdin missing")?;
        stdin.write_all(curl_config.as_bytes()).await?;
        stdin.shutdown().await?;
        drop(stdin);
        let output = child.wait_with_output().await?;
        if !output.status.success() {
            // Do not log stderr/remote bodies that might echo headers or secrets.
            return Err(format!(
                "curl transport error, exit={:?} (6=DNS,7=connect,28=timeout,60=TLS)",
                output.status.code()
            )
            .into());
        }
        let raw = String::from_utf8(output.stdout)?;
        let (body, status) = raw
            .rsplit_once("\nSTANDALONE_AI_HTTP_STATUS:")
            .ok_or("missing HTTP status")?;
        let status: u16 = status.trim().parse()?;
        parse_api_http_response(body, status, config.max_response_chars)
    };
    timeout(Duration::from_secs(seconds + 1), run).await?
}

fn escape_curl_config(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
}

fn parse_chat_response(body: &str, max_chars: usize) -> Result<String, AppError> {
    let response: ChatResponse =
        serde_json::from_str(body).map_err(|err| format!("AI 返回的不是有效 JSON: {err}"))?;

    if let Some(error) = response.error.filter(|e| !e.is_null()) {
        return Err(Box::new(classify_api_failure(200, Some(&error))));
    }

    let choices = response
        .choices
        .ok_or_else(|| "AI 响应缺少 choices".to_string())?;
    let first = choices
        .first()
        .ok_or_else(|| "AI 响应 choices 为空".to_string())?;

    let content = first
        .message
        .as_ref()
        .and_then(|message| message.content.clone())
        .or_else(|| first.text.clone())
        .unwrap_or_default();

    let content = content.trim();
    if content.is_empty() {
        return Err("AI 响应内容为空".into());
    }

    Ok(truncate(content, max_chars))
}

fn truncate(value: &str, max_chars: usize) -> String {
    let mut result = value.chars().take(max_chars).collect::<String>();
    if value.chars().count() > max_chars {
        result.push('…');
    }
    result
}

#[cfg(test)]
mod regression_tests {
    use super::*;
    #[test]
    fn common_native_and_knowledge_routes() {
        for t in [
            "打开卧室灯光",
            "把客厅空调调到26度",
            "下一首",
            "暂停",
            "播放周杰伦的歌",
            "明早七点设个闹钟",
            "提醒我喝水",
            "关闭窗帘",
            "空调开着吗",
        ] {
            assert!(is_native_control_command(t), "native: {t}");
        }
        for t in [
            "空调为什么打不开",
            "开灯原理是什么",
            "离子是什么意思",
            "推荐音乐",
            "如何设置闹钟",
            "介绍一下你自己",
            "讲一个长故事",
            "关灯睡觉有什么好处",
        ] {
            // Last example is an informational question, not an action.
            assert!(!is_native_control_command(t), "AI: {t}");
        }
    }
    #[test]
    fn ubus_code_is_parsed_not_substring_matched() {
        assert!(check_ubus_response(br#"{ "code" : 0 }"#, "tts").is_ok());
        assert!(check_ubus_response(br#"{"code":1,"info":"code:0"}"#, "tts").is_err());
        assert!(check_ubus_response(b"", "tts").is_err());
    }
    #[test]
    fn compatible_content_and_history() {
        assert_eq!(
            parse_chat_response(
                r#"{"choices":[{"message":{"content":"测试成功","reasoning_content":"hidden"}}]}"#,
                1200
            )
            .unwrap(),
            "测试成功"
        );
        assert!(
            parse_chat_response(r#"{"error":{"message":"error","code":"balance"}}"#, 1200).is_err()
        );
        let mut history = (0..30)
            .map(|_| HistoryMessage {
                role: "user".into(),
                content: "t".into(),
            })
            .collect();
        trim_history(&mut history, 10);
        assert_eq!(history.len(), 20);
    }
    #[tokio::test]
    async fn placeholders_never_take_over_native() {
        let cfg = Arc::new(Config {
            api_key: "CHANGE_ME".into(),
            ..Config::default()
        });
        let state = Arc::new(Mutex::new(State::default()));
        handle_user_text("test".into(), cfg, state.clone())
            .await
            .unwrap();
        assert!(!state.lock().await.busy);
    }
}

#[cfg(test)]
mod http_tests {
    use super::*;
    #[tokio::test]
    async fn curl_stdin_and_http_errors() {
        use std::io::{Read, Write};
        for (status, body, success) in [
            (200, r#"{"choices":[{"message":{"content":"ok"}}]}"#, true),
            (402, r#"{"error":{"message":"balance"}}"#, false),
            (429, r#"{"error":{"message":"rate"}}"#, false),
            (200, "not JSON", false),
        ] {
            let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
            let addr = listener.local_addr().unwrap();
            let server = std::thread::spawn(move || {
                let (mut socket, _) = listener.accept().unwrap();
                socket
                    .set_read_timeout(Some(Duration::from_secs(5)))
                    .unwrap();
                let mut data = Vec::new();
                let mut buf = [0; 2048];
                let header_end = loop {
                    let n = socket.read(&mut buf).unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buf[..n]);
                    if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                        break i + 4;
                    }
                };
                let headers = String::from_utf8_lossy(&data[..header_end]).to_lowercase();
                assert!(headers.contains("authorization: bearer local-test"));
                let length: usize = headers
                    .lines()
                    .find_map(|l| l.strip_prefix("content-length:"))
                    .unwrap()
                    .trim()
                    .parse()
                    .unwrap();
                while data.len() < header_end + length {
                    let n = socket.read(&mut buf).unwrap();
                    assert!(n > 0);
                    data.extend_from_slice(&buf[..n]);
                }
                let value: Value =
                    serde_json::from_slice(&data[header_end..header_end + length]).unwrap();
                assert_eq!(
                    value["messages"].as_array().unwrap().last().unwrap()["content"],
                    "quotes: \"test\"\nline\\end"
                );
                let response = format!("HTTP/1.1 {status} Test\r\nContent-Length: {}\r\nContent-Type: application/json\r\nConnection: close\r\n\r\n{body}", body.len());
                socket.write_all(response.as_bytes()).unwrap();
            });
            let config = Arc::new(Config {
                base_url: format!("http://{addr}/v1"),
                api_key: "local-test".into(),
                ..Config::default()
            });
            let result = request_chat_completion(
                "quotes: \"test\"\nline\\end",
                config,
                Arc::new(Mutex::new(State::default())),
            )
            .await;
            server.join().unwrap();
            assert_eq!(result.is_ok(), success);
            if status != 200 {
                assert!(result
                    .unwrap_err()
                    .to_string()
                    .contains(&format!("HTTP {status}")));
            }
        }
    }
    #[tokio::test]
    async fn cooldown_and_busy_requests_stay_native() {
        let cfg = Arc::new(Config {
            api_key: "local-test".into(),
            ..Config::default()
        });
        let state = Arc::new(Mutex::new(State {
            native_fallback_until: Some(Instant::now() + Duration::from_secs(300)),
            ..State::default()
        }));
        handle_user_text("知识问题".into(), cfg.clone(), state.clone())
            .await
            .unwrap();
        assert!(!state.lock().await.busy);
        state.lock().await.busy = true;
        let before = state.lock().await.generation;
        handle_user_text("打开卧室灯光".into(), cfg, state.clone())
            .await
            .unwrap();
        assert_ne!(state.lock().await.generation, before);
    }
}

#[cfg(test)]
mod balance_tests {
    use super::*;
    fn kind(status: u16, body: &str) -> ApiFailureKind {
        let err = parse_api_http_response(body, status, 1200).unwrap_err();
        err.downcast_ref::<ApiFailure>().unwrap().kind
    }
    #[test]
    fn balance_is_distinct_from_rate_limit_auth_and_quota() {
        assert_eq!(kind(402, ""), ApiFailureKind::Balance);
        assert_eq!(
            kind(429, r#"{"error":{"message":"Too many requests"}}"#),
            ApiFailureKind::RateLimitOrQuota
        );
        assert_eq!(
            kind(429, r#"{"error":{"code":"insufficient_quota"}}"#),
            ApiFailureKind::Quota
        );
        assert_eq!(
            kind(400, r#"{"error":{"message":"余额不足"}}"#),
            ApiFailureKind::Balance
        );
        assert_eq!(
            kind(200, r#"{"error":{"code":"insufficient_balance"}}"#),
            ApiFailureKind::Balance
        );
        assert_eq!(
            kind(401, r#"{"error":{"message":"invalid key"}}"#),
            ApiFailureKind::Authentication
        );
        assert_eq!(
            kind(500, "insufficient balance in unrelated HTML"),
            ApiFailureKind::Other
        );
    }
    #[test]
    fn success_content_cannot_trigger_balance_warning() {
        let text = parse_api_http_response(
            r#"{"choices":[{"message":{"content":"余额不足是什么意思"}}]}"#,
            200,
            1200,
        )
        .unwrap();
        assert_eq!(text, "余额不足是什么意思");
    }
    #[test]
    fn notices_are_rate_limited() {
        let mut state = State::default();
        let now = Instant::now();
        assert!(claim_balance_notice(&mut state, ApiFailureKind::Other, now).is_none());
        assert!(claim_balance_notice(&mut state, ApiFailureKind::Balance, now).is_some());
        assert!(claim_balance_notice(
            &mut state,
            ApiFailureKind::Balance,
            now + Duration::from_secs(301)
        )
        .is_none());
        assert!(claim_balance_notice(
            &mut state,
            ApiFailureKind::Quota,
            now + Duration::from_secs(1800)
        )
        .is_some());
    }
}
