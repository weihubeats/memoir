use crate::model::Entry;
use crate::{profile, repo, search, Ctx};

pub const DEFAULT_BUDGET_CHARS: usize = 6000;

fn take_within(entries: Vec<Entry>, budget: usize) -> Vec<Entry> {
    let mut out = Vec::new();
    let mut used = 0usize;
    for e in entries {
        let cost = e.title.chars().count() + e.body.chars().count();
        if !out.is_empty() && used + cost > budget {
            break;
        }
        used += cost;
        out.push(e);
    }
    out
}

/// 注入策略的落地（设计文档 §4）：档案常驻 + 按需检索历史，总量受字符预算约束。
/// 带 query 走混合检索，不带 query 用最近记录。
pub async fn get_context(
    ctx: &Ctx,
    query: Option<&str>,
    budget_chars: usize,
) -> anyhow::Result<String> {
    let profile_text = profile::get_profile(ctx, profile::ProfileOptions::default()).await?;
    let mut out = format!("## 个人档案\n\n{profile_text}");
    let used = out.chars().count();

    let remaining = budget_chars.saturating_sub(used);
    let entries = match query.map(str::trim).filter(|q| !q.is_empty()) {
        Some(q) => search::search_entries(ctx, q, search::DEFAULT_TOP_K, remaining).await?,
        _ => take_within(repo::recent_entries(ctx, 10).await?, remaining),
    };

    out.push_str("\n\n## 相关历史\n\n");
    if entries.is_empty() {
        out.push_str("（无）\n");
    }
    for e in entries {
        out.push_str(&format!(
            "- #{} [{}] {} {}\n",
            e.id, e.kind, e.occurred_at, e.title
        ));
        if !e.body.is_empty() {
            out.push_str(&format!("  {}\n", e.body.replace('\n', "\n  ")));
        }
    }
    Ok(out)
}
