use crate::ctx::Ctx;

#[derive(Debug, Clone, Default)]
pub struct ProfileOptions {
    /// 只看这些域（None = 全部）
    pub domains: Option<Vec<String>>,
    /// 是否包含 sensitive 级事实（话题相关时由 AI 显式拉取）
    pub include_sensitive: bool,
    /// 导出场景用：连 secret 一起出
    pub include_secret: bool,
}

#[derive(sqlx::FromRow)]
struct ProfileRow {
    entity_name: String,
    key: String,
    value: String,
    domain: Option<String>,
    sensitivity: String,
    valid_from: String,
}

/// 档案摘要：当前有效事实按域分组的紧凑文本，可直接作为注入上下文。
/// secret 级除显式 include_secret（导出）外永不出现。
pub async fn get_profile(ctx: &Ctx, opts: ProfileOptions) -> anyhow::Result<String> {
    let rows = sqlx::query_as::<_, ProfileRow>(
        "SELECT e.name AS entity_name, f.key, f.value, f.domain, f.sensitivity, f.valid_from \
         FROM facts f JOIN entities e ON e.id = f.entity_id \
         WHERE f.valid_to IS NULL ORDER BY f.domain, e.name, f.key",
    )
    .fetch_all(&ctx.pool)
    .await?;

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    for r in rows {
        match r.sensitivity.as_str() {
            "secret" if !opts.include_secret => continue,
            "sensitive" if !opts.include_sensitive => continue,
            _ => {}
        }
        let domain = r.domain.clone().unwrap_or_else(|| "general".to_owned());
        if let Some(want) = &opts.domains {
            if !want.is_empty() && !want.contains(&domain) {
                continue;
            }
        }
        if domain != current {
            lines.push(format!("# {domain}"));
            current = domain;
        }
        lines.push(format!(
            "- {} | {}: {}（自 {}，{}）",
            r.entity_name, r.key, r.value, r.valid_from, r.sensitivity
        ));
    }
    if lines.is_empty() {
        return Ok("（档案为空）".to_owned());
    }
    Ok(lines.join("\n"))
}
