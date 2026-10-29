//! 内置对话（M4）：LLM 流式客户端（OpenAI 兼容）+ 写操作确认卡解析。
//!
//! AI 通过 ```memoir-op 围栏输出写操作 JSON，客户端解析成确认卡片，
//! 经用户采纳才真正落库——对话流中绝不静默写入。

use serde::{Deserialize, Serialize};

/// OpenAI 兼容配置：云端（GLM/OpenAI/DeepSeek 等）与本地（Ollama /v1）通用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmConfig {
    pub base_url: String,
    pub api_key: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmMessage {
    pub role: String,
    pub content: String,
}

/// AI 提出的写操作。
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "type")]
pub enum ChatOp {
    #[serde(rename = "upsert_fact")]
    UpsertFact {
        entity: String,
        key: String,
        value: serde_json::Value,
        #[serde(default)]
        domain: Option<String>,
        #[serde(default)]
        sensitivity: Option<String>,
    },
    #[serde(rename = "add_entry")]
    AddEntry {
        title: String,
        body: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        tags: Vec<String>,
        #[serde(default)]
        entities: Vec<String>,
    },
}

/// 助手消息解析后的渲染段。
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Segment {
    Text { content: String },
    Op { index: usize, op: ChatOp },
}

pub const OP_FENCE: &str = "```memoir-op";

pub const SYSTEM_PROMPT: &str = r#"你是 Memoir——用户的个人上下文助手。上下文中注入了用户的档案摘要与相关历史，请基于它们回答；档案里没有的信息不要臆测，可以询问用户。

当用户提到新的持久信息或想记录内容时，不要声称「已记录」——输出一个 ```memoir-op 代码块（JSON），应用会渲染确认卡片，经用户采纳后才真正入库：

```memoir-op
{"type":"upsert_fact","entity":"我","key":"weight","value":"70kg","domain":"health","sensitivity":"normal"}
```

或：

```memoir-op
{"type":"add_entry","title":"标题","body":"正文","kind":"event","tags":[],"entities":["我"]}
```

规则：持久个人信息（身高体重、就职、收入资产、家人情况、偏好目标）用 upsert_fact；一次性事件与感受用 add_entry；sensitivity 默认 normal，收入数额/持仓明细/健康异常/感情状况用 sensitive，卡号/证件号/账号密码等标识符用 secret。每次对话结束时，把本次值得记录的新信息用 memoir-op 提出。"#;

/// 解析助手消息：```memoir-op 围栏 → 确认卡段，其余为文本段；解析失败按原文回显。
pub fn parse_segments(text: &str) -> Vec<Segment> {
    let mut segments: Vec<Segment> = Vec::new();
    let mut rest = text;
    loop {
        let Some(start) = rest.find(OP_FENCE) else {
            if !rest.is_empty() {
                segments.push(Segment::Text {
                    content: rest.to_owned(),
                });
            }
            return segments;
        };
        if start > 0 {
            segments.push(Segment::Text {
                content: rest[..start].to_owned(),
            });
        }
        let after = &rest[start + OP_FENCE.len()..];
        let Some(end) = after.find("```") else {
            segments.push(Segment::Text {
                content: rest[start..].to_owned(),
            });
            return segments;
        };
        let raw = after[..end].trim();
        match serde_json::from_str::<ChatOp>(raw) {
            Ok(op) => {
                let index = segments
                    .iter()
                    .filter(|s| matches!(s, Segment::Op { .. }))
                    .count();
                segments.push(Segment::Op { index, op });
            }
            Err(_) => segments.push(Segment::Text {
                content: format!("{OP_FENCE}\n{raw}\n```"),
            }),
        }
        rest = &after[end + 3..];
    }
}

/// 流式对话：SSE 增量回调 + 返回完整回复。base_url 指向 OpenAI 兼容端点。
pub async fn chat_stream(
    cfg: &LlmConfig,
    messages: &[LlmMessage],
    mut on_delta: impl FnMut(&str),
) -> anyhow::Result<String> {
    let client = reqwest::Client::new();
    let url = format!("{}/chat/completions", cfg.base_url.trim_end_matches('/'));
    let mut resp = client
        .post(url)
        .bearer_auth(&cfg.api_key)
        .json(&serde_json::json!({
            "model": cfg.model,
            "messages": messages,
            "stream": true,
        }))
        .send()
        .await?;
    let status = resp.status();
    if !status.is_success() {
        let body = resp.text().await.unwrap_or_default();
        let excerpt: String = body.chars().take(300).collect();
        anyhow::bail!("LLM 请求失败 {status}：{excerpt}");
    }

    let mut full = String::new();
    let mut buffer = String::new();
    while let Some(chunk) = resp.chunk().await? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(pos) = buffer.find('\n') {
            let line: String = buffer[..pos].trim_end_matches('\r').to_owned();
            buffer.drain(..=pos);
            let Some(data) = line.strip_prefix("data: ") else {
                continue;
            };
            let data = data.trim();
            if data == "[DONE]" || data.is_empty() {
                continue;
            }
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(data) {
                if let Some(delta) = v["choices"][0]["delta"]["content"].as_str() {
                    if !delta.is_empty() {
                        full.push_str(delta);
                        on_delta(delta);
                    }
                }
            }
        }
    }
    Ok(full)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_text_and_ops() {
        let text = concat!(
            "你的体重我记一下。\n\n",
            "```memoir-op\n",
            r#"{"type":"upsert_fact","entity":"我","key":"weight","value":"70kg","domain":"health"}"#,
            "\n```\n\n还有别的吗？",
        );
        let segments = parse_segments(text);
        assert_eq!(segments.len(), 3);
        assert!(matches!(&segments[0], Segment::Text { content } if content.contains("体重")));
        match &segments[1] {
            Segment::Op { index, op } => {
                assert_eq!(*index, 0);
                assert_eq!(
                    *op,
                    ChatOp::UpsertFact {
                        entity: "我".into(),
                        key: "weight".into(),
                        value: json!("70kg"),
                        domain: Some("health".into()),
                        sensitivity: None,
                    }
                );
            }
            _ => panic!("应为 Op 段"),
        }
        assert!(
            matches!(&segments[2], Segment::Text { content } if content.contains("还有别的吗"))
        );
    }

    #[test]
    fn multiple_ops_get_incrementing_index() {
        let text = concat!(
            "```memoir-op\n",
            r#"{"type":"add_entry","title":"a","body":"b"}"#,
            "\n```\n",
            "```memoir-op\n",
            r#"{"type":"add_entry","title":"c","body":"d"}"#,
            "\n```",
        );
        let segments = parse_segments(text);
        assert_eq!(
            segments
                .iter()
                .filter_map(|s| match s {
                    Segment::Op { index, .. } => Some(*index),
                    _ => None,
                })
                .collect::<Vec<_>>(),
            vec![0, 1]
        );
    }

    #[test]
    fn broken_op_falls_back_to_text() {
        let text = "```memoir-op\nnot json\n```";
        let segments = parse_segments(text);
        assert!(matches!(&segments[0], Segment::Text { content } if content.contains("not json")));
    }

    #[test]
    fn unterminated_fence_is_text() {
        let text = "开头\n```memoir-op\n{\"type\":\"add_entry\"";
        let segments = parse_segments(text);
        assert!(!segments.iter().any(|s| matches!(s, Segment::Op { .. })));
        assert!(segments
            .iter()
            .any(|s| matches!(s, Segment::Text { content } if content.contains("开头"))));
    }
}
