---
title: 什么是 MAP？
description: Mixum Agent Protocol——目的、生态、设计原则与术语。
---

MAP（**Mixum Agent Protocol**）是一个开放协议，把 AI agent 连接到**可隔离、可快照的计算环境**——也就是 agent 所生活的那个「世界」——横跨 macOS、Windows、Linux 宿主，从本地笔记本到云端。

只能调用工具的 agent，依然被困在别人的沙箱里。MAP 让沙箱本身变得可互操作：

- **带隔离的任务执行**——在 VM 或容器（MAP *Node*）里干活，进程级沙箱，外加保证性的任务前置快照。
- **时间旅行**——每个 Node 暴露一棵快照树：回退到任一时间点、并行时间线、分叉实验。机器状态的 git 语义。
- **可迁移**——把完整工作状态（文件，可选地含活进程内存）在位于不同机器的 Node 之间搬动：本地↔云端，幂等且廉价。
- **窗口与交互**——agent 打开的 GUI 应用以原生窗口流到任意客户端，接管状态机语义明确。
- **Agent 记忆**——一套归一化的记录 schema 加导入导出适配器，记忆跟着 agent 跨 Node、跨平台。

## 问题所在

今天，每个 agent 沙箱厂商都在为同样一小撮需求发明私有 API：跑任务、看过程、存取状态、搬工作。客户端被锁死在一个后端，agent 被锁死在一个运行时。MAP 定义了一个小而严谨、能力令牌守卫、带版本协商的表面，让**任何客户端**都能驱动**任何合规的 Node**。

MCP 标准化了 agent↔工具这条边，MAP 标准化 agent↔环境这条边，两者互补：MCP server 可以跑在 MAP Node 里；MAP Node 可以承载任何会说 MAP-Bridge 的 agent。

## 生态

| 项目 | 角色 | MAP 一致性 |
|---|---|---|
| [mixumOS](https://github.com/ashreo/mixumOS) | 系统基座：基于 Arch 的 Agent OS 发行版。跑着 `mixum-agentd` + `mixcomp` 的容器/VM 是参考 **Node** 实现。 | Core、Display、Mobility（参考实现） |
| **Mixum Agent Protocol**（本仓库） | 契约本身：正典规范、JSON Schema、一致性向量、SDK。 | — |
| [Mixum Agent Env (MixumART)](https://github.com/ashreo/mixum-Agent-Env) | 桌面 GUI 与配套环境：macOS/Windows/Linux 上的 VM 生命周期管理、agent 平台连接器。参考 **Host** + **Client**。 | Host、Core client |

## 设计原则

1. **基于能力的安全。** 每条连接只持有最小权限的签名令牌。没有环境权限，没有"已登录用户"——只有能力。
2. **一切内容寻址。** 快照块与清单以明文 SHA-256 寻址。压缩与加密只是存储细节；去重天然全局，哈希校验全量覆盖。
3. **事件溯源的状态。** 任务与快照树都是可观察的事件流。客户端断线重连后可以追平；服务端是唯一事实源。
4. **协商式、增量的演进。** 版本在 hello 时协商。增量变更（新 op、可选字段、能力、编码）是次版本；破坏性变更是大版本，且尽量罕见。
5. **Profile 分级，而非单体。** 实现按 **MAP-Core** 加可选 Profile（Display、Mobility、Memory、Host）声明一致性。客户端在缺少 Profile 时优雅降级。
6. **语言中立的上星。** JSON 控制面 + 带标签的二进制帧，由 JSON Schema 和可执行一致性向量锁死。SDK 只是便利品，永远不是定义。

## 术语

| 术语 | 定义 |
|---|---|
| **Node** | VM 或容器内的沙箱运行时（如 mixumOS）。承载任务、快照、显示、记忆。 |
| **Host** | 桌面侧守护进程（如 MixumART）：管理本地 VM，向客户端分发 Node 的引导访问。 |
| **Client** | 协议的消费方：GUI、手机 PWA、CLI，或另一个 agent。 |
| **Peer** | 与本 Node 交换快照数据的另一个 Node（push/pull）。 |
| **Gateway** | 云侧路由与多租户层。预留；v1 不涉及。 |
| **Task** | 一单元 agent 工作：指令、生命周期、事件流、任务前置快照。 |
| **Snapshot (SnapNode)** | 工作区和/或任务进程树在某一时刻的状态，是快照树上的一个节点。 |
| **Branch / Head** | 分支是快照节点的属性；head 是指针，指向下一张快照的挂载点。 |
| **CAS** | 内容寻址存储：块以明文 SHA-256 为键。 |
| **Manifest** | 快照的文件清单：路径、权限、大小、块列表。 |
| **Capability token** | `map1.<payload>.<sig>`——ed25519 签名的能力集合，带 TTL 与可选绑定。 |
| **Profile** | 命名的一致性分级：MAP-Core、Display、Mobility、Memory、Host。 |
| **Bridge (MAP-Bridge)** | Node 的 runner 与外部 agent 进程之间的正典行分隔 JSON 子协议。 |
| **Window** | 沙箱中暴露出来供流式观看与（受控）输入的 GUI 窗口。 |
| **Memory record** | 一单元归一化的 agent 记忆：provider、kind、scope、content。 |

## 非目标（v1）

- **多租户与公有云路由**——Gateway 预留给未来版本；v1 是单用户多设备。
- **Agent 间协议**——请在 Node 内使用 MCP 等；MAP 承载环境，不承载 agent 之间的对话。
- **特定的 agent 框架**——MAP-Bridge 刻意极小，任何框架一个下午就能接上。
- **浏览器侧沙箱**——Node 跑在真实宿主的 VM/容器里。
