---
title: 记忆（Memory）
description: 可移植的 agent 记忆——记录、provider、搜索、导入导出与跨节点同步。
---

稳定性：**v1.0 转 Stable**（Profile **MAP-Memory**；跨节点实时同步为 Experimental）。

Agent 会积累上下文——偏好、事实、操作步骤、事件历史。今天这些记忆被锁在各个 agent 平台里。MAP 把它变成**一等公民、可移植的产物**：一套归一化 schema，记忆跟着 agent 跨 Node 走，平台记忆可以「强制兼容」地导入而不是作废。

## 目标

1. **可移植**——记录在 Node 之间流动（经[可迁移性](/map/zh/spec/mobility/)或导入导出）。
2. **平台互操作**——主流平台的记忆由导入器转成 MAP 记录；协议本身保持中立。
3. **用户所有**——记忆存在用户自己的 Node，不在厂商云里。隐私是架构性的。

## 模型

- **Provider** 是记忆的命名来源/仓库：`builtin`（Node 本地库）、`platform-import:chatgpt`、`platform-import:claude`、`remote:*`，或实现自定义的。
- **Record** 是一单元归一化记忆：

```json
{
  "id": "01JBVB6Q…",
  "provider": "builtin",
  "kind": "preference",
  "content": {
    "text": "偏好简洁回答，不要开场白。",
    "structured": { "locale": "zh-CN", "verbosity": "low" }
  },
  "scope": { "level": "global" },
  "source": { "platform": "chatgpt", "origin": "import 2026-10-18" },
  "tags": ["style"],
  "createdAt": 1696400000123,
  "updatedAt": 1696400000123,
  "confidence": 0.9,
  "expiresAt": null,
  "opaque": null
}
```

| 字段 | 规则 |
|---|---|
| `id` | ULID；`mem.put` 缺省时由服务端分配 |
| `provider` | 必须存在于本 Node |
| `kind` | `fact` \| `preference` \| `procedure` \| `episodic` \| `custom` |
| `content.text` | 必填的人类可读文本（注入 prompt 的就是它） |
| `content.structured` | 可选的键值细化 |
| `scope.level` | `global` \| `project` \| `task`；`scope.ref` = 项目根或任务 ULID |
| `source` | 来源追溯；导入记录的 `platform` 指明出处平台 |
| `opaque` | provider 专属 blob（base64），往返保留——**保证任何导入器的数据不丢** |

## 操作

| Op | 请求 | 应答 |
|---|---|---|
| `mem.providers` | — | `{ providers: [{ id, kind, status, capabilities }] }`——capabilities ⊆ `read,write,search,import,export` |
| `mem.search` | `{ query, kinds?, providers?, scope?, limit? }` | `{ hits: [{ record, score }] }` |
| `mem.get` | `{ id }` | Record |
| `mem.put` | `{ record }` | Record（服务端分配 `id`/`updatedAt`） |
| `mem.delete` | `{ id }` | `{ deleted }` |
| `mem.list` | `{ provider?, kind?, cursor?, limit? }` | `{ records, nextCursor? }` |
| `mem.import` | `{ provider, format: "map-json", records: [Record] }` | `{ imported, skipped }` |
| `mem.export` | `{ provider?, format: "map-json" }` | `{ records: [Record] }` |

说明：

- **搜索排序在 v1 由实现定义**（子串/全文/向量皆合规）；`score` 归一化 0–1，按相关度降序。带语义搜索的 Node 在 `mem.providers` 里把 provider 能力标 `search` 并置 `searchSemantic: true`。
- `mem.put` 遇到已有 `id` 即**更新**（最后写入者胜）；离线编辑的客户端应读-改-写。
- `mem.import` 按 `id` upsert；无 `id` 的记录总是新插入。`skipped` 计数校验被拒的记录。
- `format` 在 v1 只有 `"map-json"`；注册表将来可加具名格式（如 `"chatgpt-export"`）——但平台专属转换属于 **SDK/Host**（[见下](#平台兼容)），不属于 Node。

## 存储与可迁移性

- 记忆库在 Node 本地（`dataDir/memory`），**在工作区之外**：默认不随 `disk` 快照走。
- 它的流动靠显式选择：`snap.push { includeMemory: true }` 让记忆库的块随行（[可迁移性](/map/zh/spec/mobility/)），或者 `mem.export` / `mem.import` 往返搬运。
- 逐记录 ACL 与静态加密为预留项（v1.1）；记忆库继承 Node 的静态加密姿态。

## 平台兼容

MAP 早期对主流 agent 平台采取**强制兼容**。分工如下：

| 层 | 职责 |
|---|---|
| **导入器**（SDK / MixumART 内） | 把平台导出（ChatGPT memory、Claude memory、Cursor rules、…）转成 `map-json` 记录；原始数据存 `opaque` |
| **协议** | 搬运与存储归一化记录；靠 `opaque` 保证无损往返 |
| **Node** | 暴露 provider；在本地 agent runner 能消费的场合套用记录 |

导入器覆盖清单记录在 SDK 文档里，不进规范——规范只要求「导入器可表达」，`opaque` + `source.platform` 正是为此。

## Experimental：实时同步

`mem.sync { remote, since? }`（Node 间持续记录复制）在 v1 为 **Experimental**：次版本内可能变更，不参与一致性。Node 不得默认开启。
