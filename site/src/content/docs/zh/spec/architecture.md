---
title: 架构与一致性分级
description: 角色、拓扑、一致性 Profile 与能力模型。
---

## 角色

MAP 分四个角色。一个进程可以同时扮演多个（Host 通常也是它自己 Node 的 Client；Node 互为 Peer）。

```
        ┌──────────────┐   ┌──────────────┐   ┌──────────────┐
        │ Client (GUI, │   │ Client (CLI, │   │ Client (PWA) │
        │  MixumART)   │   │  SDK, agent) │   │              │
        └──────┬───────┘   └──────┬───────┘   └──────┬───────┘
               │   MAP (WSS)      │                  │
        ┌──────▼──────────────────▼──────────────────▼───────┐
        │                  Host (MAP-Host 角色)              │
        │        vm.*：创建/启停 VM，发放引导令牌             │
        └──────┬──────────────────────────┬──────────────────┘
               │ MAP (WSS, 端口转发)       │
        ┌──────▼───────┐           ┌──────▼───────┐
        │  Node (VM)   │   ⇄ CAS   │ Node (云端    │
        │  mixumOS     │    同步    │ pod / VM)    │
        │  task/snap/  │           │ 同一套 MAP    │
        │  display/mem │           │ 表面          │
        └──────────────┘           └──────────────┘
```

| 角色 | 运行于 | 提供什么 | 必须实现 |
|---|---|---|---|
| **Node** | VM 或容器内（当前为 Linux） | `session`、`sys`、`task`、`snap`、`agent`（bridge），可选 `display`、`mem` | MAP-Core |
| **Host** | 桌面：macOS / Windows / Linux | 对其客户端暴露 `vm.*`；代理或转发到其 Node | MAP-Host |
| **Client** | 任意 | 消费方；主动发起连接 | MAP-Core 客户端侧 |
| **Peer** | 同 Node | `snap` push/pull 对端 | MAP-Mobility |
| **Gateway** | 云 | 路由、多租户鉴权 | 预留（v1 不含） |

## 拓扑规则

- 客户端**直连** Node（`wss://`，TLS 1.3 + TOFU 指纹固定）。Host 的 `vm.attach` 是发现 Node 的 URL、证书指纹与引导令牌的标准途径。
- Node 间的快照同步是对等的：推送方 Node 作为接收方 Node 的 MAP **客户端**，持 `snap.sync` 能力令牌。
- Host **永不**身处窗口帧或数据块的传输路径；它只是控制面的便利层。直连的客户端↔Node 路径才是常态。

## 一致性 Profile

实现声明自己实现了哪些 Profile；`hello.ok` 会回告这个集合。缺少可选 Profile 时客户端必须优雅降级。

| Profile | 内容 | v1 状态 |
|---|---|---|
| **MAP-Core** | Envelope 与分帧、session 与 sys、任务、快照树基础（create/restore/fork/delete/gc）、MAP-Bridge 存在性 | Stable |
| **MAP-Display** | `display.*`、窗口流、输入、接管 | Stable |
| **MAP-Mobility** | `snap.push`/pull、CAS 同步协议、清单交换、恢复守卫 | Stable |
| **MAP-Memory** | `mem.*`、记忆记录、provider 导入导出 | v1.0 时转 Stable |
| **MAP-Host** | `vm.*`、镜像、引导交接 | v1.0 时转 Stable |
| MAP-Cluster | 经 Gateway 的多节点调度 | 预留 |

一致性按范围声明：实现只声称自己通过向量测试的范围，例如「MAP v1 Core + Display conformant」。见[版本与治理](/map/zh/spec/versioning-governance/)。

## 能力模型

鉴权完全基于能力。一条连接只拥有令牌授予的能力——多一点都不行。服务端必须把缺少能力处理为 `forbidden`，绝不能崩溃或静默忽略。

| 能力字符串 | 授予 |
|---|---|
| `task.run` | 提交与取消任务 |
| `task.observe` | 订阅任务事件、列出/读取任务、快照与系统信息 |
| `snap.manage` | 创建、恢复、分叉、删除快照；执行 GC |
| `snap.sync` | 接收推送的快照（作为同步目标） |
| `display.view` | 列窗口、附着到流 |
| `display.input` | 注入输入、接管 |
| `mem.read` | 搜索、列出、读取、导出记忆记录 |
| `mem.write` | 写入、删除、导入记忆记录 |
| `vm.manage` | 创建、启停、删除 VM 与镜像（Host 角色） |
| `admin` | 全部能力；用于引导与派生受限令牌 |

令牌 claims 可以进一步**绑定**能力：`taskId`（只能操作该任务，同时禁止提交新任务）、`windows`（只能操作这些窗口）、`mem.providers`（只能访问这些记忆 provider）。见[传输与安全](/map/zh/spec/transport-security/)。

典型授予：

| 调用方 | 令牌能力 |
|---|---|
| 桌面 GUI / 手机 PWA | `task.run`、`task.observe`、`display.view`、`display.input` |
| 快照同步对端 | `snap.sync`（短 TTL，按远端签发） |
| 无头自动化客户端 | `task.run`、`task.observe`、`snap.manage` |
| Agent 进程（经 runner） | 无——agent 只通过 MAP-Bridge 与 runner 对话，不直接上公网线 |
