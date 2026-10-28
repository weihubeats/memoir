# my-context 设计文档

> 状态：v0.1 草案（2026-10-29）

## 1. 目标

把一个人所有的关键个人信息集中存放在自己可控的地方（本地优先），并在与 AI 大模型对话时让 AI 按需读取这些信息，从而给出真正「了解你」的建议。

上下文范围举例：

- 基础生理：身高、体重、体检指标、作息习惯
- 财务：收入、储蓄、各银行卡、负债、持仓（股票/基金）、大额支出计划
- 职业：在职公司、岗位、技能、薪酬、职业意向
- 家庭与社会关系：父母状况、感情状态、重要亲友
- 偏好与原则：饮食口味、风险偏好、决策原则、长期目标

**非目标（现阶段）**：不做多人共享账户；不做自动抓取（爬网银/券商）；不做公网多租户服务。

## 2. 核心概念与数据模型

关键设计决策：**把「事实」与「记录」分开存**。

- **事实（Fact）**：结构性、会随时间变化的状态。如「身高 175cm」「就职于 X 公司」「持有 Y 股票 2000 股」。带生效时间区间，旧值保留历史。
- **记录（Entry）**：只追加的事件与叙事。如一次体检、一次谈话、一段日记、一次重要决策及其理由。永不修改，只追加。

### 2.1 模型

```
domain    主题域：health / wealth / career / family / relationship / preference / goal
entity    实体：我、某人（父母/伴侣）、某公司、某银行卡、某持仓……
fact      事实：entity + key + value(JSON) + valid_from/valid_to + source_entry_id + sensitivity
entry     记录：时间 + 类型(日记/事件/对话摘录/导入) + 标题 + Markdown 正文 + 关联实体 + 标签
embedding 向量：entry 的语义嵌入（M2 起）
```

示例：

- fact：entity=我, key=employer, value={"company":"X公司","title":"后端工程师"}, valid_from=2024-03, valid_to=2026-10
- fact：entity=招商银行尾号1234, key=account_type, value="储蓄卡", sensitivity=sensitive
- entry：2026-10-20「和爸妈视频，爸血压偏高，提醒他复查」

### 2.2 敏感分级（sensitivity）

- `normal`：可随任意对话注入（饮食偏好、职业方向）
- `sensitive`：仅当话题明确相关才注入（收入、健康异常、持仓明细）
- `secret`：默认永不注入，需用户在当次对话显式授权（卡号、具体存款数额）

## 3. 系统形态

本地优先的单机系统，一条核心链路：

```
SQLite(单文件) ←→ Rust 核心(存储/检索/注入策略) ←→ MCP Server ←→ AI 客户端(Claude/ZCode/…)
```

- **v1 不自建聊天 UI**：以 MCP server 接入现成 AI 客户端，最快跑通「AI 读取全部上下文」的核心价值。
- **v2 再做 Tauri + Vue 桌面端**：档案管理、时间线、过期事实审查（可复用 Miyo 已验证的模式）。
- **云同步（可选，靠后）**：本地单文件库端到端加密后整体同步（私有对象存储 / WebDAV / 私有 Git 仓库），绝不以明文上云。

## 4. MCP 工具清单（v1）

读取：

- `get_profile(domains?, include_sensitive?)`：取「档案摘要」——各域当前有效事实的紧凑视图
- `search_entries(query, time_range?, domains?, top_k)`：关键词 + 语义混合检索历史记录
- `recent_entries(n)`：最近 n 条记录

写入（让「对话即采集」成立）：

- `add_entry(title, body, entity_ids?, tags?)`
- `upsert_fact(entity, key, value, sensitivity?)`：更新事实，旧值自动失效并保留历史
- `add_entity(name, kind, domain)`

注入策略（核心内实现，客户端无感）：

1. 每轮对话先注入「档案摘要」——全部 `normal` 事实，约 1–2KB（一个人一生的结构化事实其实很小，可以常驻）
2. AI 按话题调用 `search_entries` 拉取相关历史记录
3. `sensitive` 事实仅在话题相关时由 AI 显式拉取；`secret` 级需用户当次授权
4. 注入总量设预算上限（如 8K tokens），超限按 相关度 × 时间衰减 截断

## 5. 存储与检索选型

| 需求 | 选型 | 理由 |
|---|---|---|
| 主存储 | SQLite（sqlx） | 单文件、零运维、易备份/导出；团队已熟练 |
| 全文检索 | SQLite FTS5 | 内建，中文可配 jieba 类分词扩展 |
| 语义检索 | sqlite-vec | 单库内做向量，不引额外服务 |
| 嵌入模型 | 本地小模型（如 bge-small-zh） | 隐私：嵌入计算不出本机 |

否决的备选：纯 Markdown 文件库（事实的时间版本化与一致性难维护，量大了检索差）；服务端多租户架构（隐私风险与运维成本，与「数据主权」目标冲突）。

## 6. 数据怎么进来、怎么保持新鲜

录入优先级（从省力到费力）：

1. **对话中顺带更新**（主力）：用户随口说「我跳槽去字节了」，AI 自动调用 `upsert_fact` + `add_entry` 落库
2. **结构化编辑**：桌面端表单（M3）
3. **批量导入**：思源笔记、聊天记录导出、体检报告 PDF（M3+）

保鲜机制：

- 每条 fact 带 `updated_at` 与来源 entry；超过 N 个月未确认的事实标记「待确认」
- 每月让 AI 生成「上下文月报」：列出过期/矛盾事实请用户确认，并把当月记录聚合成摘要
- 冲突处理：新事实覆盖旧值但不删除——`valid_to` 封口，历史永远可查

备份与导出：

- `export` 命令一键导出全库为 JSON + Markdown（数据主权兜底）
- 定期自动备份 SQLite 文件到本地多副本；敏感数据不做裸网盘同步

## 7. 安全与隐私

- 本地库以 OS 磁盘加密（FileVault/BitLocker）为底，应用层对 `secret` 级字段再加密（可选 SQLCipher）
- 发往云端大模型的内容受注入策略约束：`secret` 级默认永不出机；若要求绝对不出网，可切换本地模型（Ollama）作为对话后端
- MCP server 只走本机 stdio，不开网络端口

## 8. 技术选型与理由

- **Rust 单仓库**（core + mcp + cli，后期加 tauri）：与既有栈一致，`rmcp` 官方 SDK 成熟
- **先 CLI + MCP，后 Tauri UI**：核心价值在数据与链路，UI 是管理加速器而非前置条件

## 9. 里程碑

- **M1 最小闭环**：SQLite schema + 迁移；CLI（init/add/export）；MCP server（get_profile / search_entries / add_entry / upsert_fact）；接入 Claude/ZCode 完成一次真实对话
- **M2 检索与采集增强**：FTS5 + 向量混合检索；注入预算；对话抽取事实的提示词打磨；月报生成
- **M3 桌面端**：Tauri + Vue 管理界面（档案 / 时间线 / 待确认审查 / 导入）
- **M4 可选云同步**：端到端加密同步、多端
