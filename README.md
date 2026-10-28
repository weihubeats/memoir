# my-context

个人上下文中枢（Personal Context Hub）：把个人的所有信息集中存在一个地方，与 AI 对话时由 AI 读取这些上下文，让 AI 真正「认识你」。

## 为什么

现有 AI 产品的记忆彼此孤立：换一个模型、换一个客户端，就要重新积累上下文。长期记忆是当前 AI 助手体验的核心瓶颈。my-context 的目标是把「上下文」的所有权还给用户——由用户自己持有、自己管理，任何 AI 都可以来读。

## 核心理念

- **一个地方，全部上下文**：笔记、日程、项目、经历、偏好、对话记录……统一存储、统一建模。
- **AI 读取而非用户投喂**：对话时 AI 按需检索全部个人上下文，而不是用户手动复制粘贴。
- **数据主权**：本地优先、隐私可控、随时可导出，不锁定在某一家 AI 产品里。

## 规划（Roadmap）

详细设计见 [docs/DESIGN.md](docs/DESIGN.md)。

- **M1 最小闭环**（已落地）：SQLite 数据模型 + Rust CLI + MCP server（档案读取 / 记录检索 / 事实更新），接入现有 AI 客户端跑通「AI 读取全部上下文」。
- **M2 检索与采集**（已落地）：FTS5 + 向量混合检索、注入预算控制、对话中顺带更新事实、月度上下文报告。
- **M3 桌面端（管理）**：档案总览 / 实体详情 / 时间线 / 录入与导入 / 审查中心 / 回收站 / 审计日志 / 设置。
- **M4 内置对话**：桌面端自带会话，档案注入 + AI 写操作确认卡 + 本地模型切换。
- **M5 可选云同步**：端到端加密同步，多端使用。

## 目录

```
docs/DESIGN.md        设计文档（数据模型 / 架构 / 注入策略 / 管理机制 / 桌面端功能）
crates/core           核心库：SQLite 存储 / 事实版本化 / 档案注入 / 混合检索 / 导出
crates/mcp            MCP server（rmcp，stdio）
crates/cli            命令行：my-context init / add / list / search / report / export / serve
crates/app            桌面端（Tauri 2）：档案总览 / 时间线 / 审查中心 / 设置
frontend              桌面端前端（Vue 3 + Vite + Tailwind v4 + Pinia，zh/en i18n）
```

## 快速开始（M1）

```bash
cargo build
# 初始化本地库（macOS 位于 ~/Library/Application Support/my-context/，可用 MY_CONTEXT_DB 覆盖路径）
./target/debug/my-context init

# 录入
./target/debug/my-context add fact --entity 我 --key height --value 175cm --domain health
./target/debug/my-context add entry --title "体检完成" --body "血脂偏高，建议控制饮食。" --entities 我

# 查看 / 检索 / 月报 / 导出
./target/debug/my-context list facts
./target/debug/my-context search "体检报告"
./target/debug/my-context report
./target/debug/my-context export --format markdown --out backup.md

# 作为 MCP server 接入 AI 客户端（stdio）
./target/debug/my-context serve
```

桌面端（管理界面）：

```bash
npm --prefix frontend install
npm --prefix frontend run build   # cargo 编译前需要 dist 产物存在
cargo run -p my-context-app       # 开发热更新：npm --prefix frontend run dev 另开终端
```

可选——向量语义检索（本机 Ollama，未配置则只用关键词检索）：

```bash
ollama pull bge-m3
export MY_CONTEXT_EMBED_MODEL=bge-m3
./target/debug/my-context embed   # 为存量记录补 embedding
```

Claude / ZCode 配置片段（路径换成绝对路径）：

```json
{
  "mcpServers": {
    "my-context": { "command": "/path/to/my-context", "args": ["serve"] }
  }
}
```

## 状态

技术选型已定：本地优先，Rust + SQLite（FTS5 / 向量）+ MCP；聊天 UI 后置。欢迎在 Issue 中讨论。
