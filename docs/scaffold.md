# 脚手架

![生成项目](assets/scaffold.png)

脚手架是一个编译后的 `zant` 工具，不使用 shell 脚本。

先构建框架：

```bash
cd /path/to/zant-ui
zig build
```

## 创建项目

```bash
cd /path/to/work
/path/to/zant-ui/zig-out/bin/zant create my-app
```

会询问：

1. 应用标题
2. 窗口风格：原生标题栏 / 隐藏标题栏 / 无边框
3. Apple Bundle ID
4. 是否启用 App Sandbox
5. 打包开发者账号（自动列出本机 codesign 身份；0 = ad-hoc）

生成过程会逐步输出状态，不再出现长时间无输出。

非交互：

```bash
/path/to/zant-ui/zig-out/bin/zant create my-app \
  --yes \
  --title "My App" \
  --style hidden-titlebar \
  --bundle-id com.acme.myapp \
  --sandbox true \
  --sign "Developer ID Application: Your Name (TEAMID)"
```

## 运行

```bash
cd my-app
/path/to/zant-ui/zig-out/bin/zant run
```

`zant run` 会读取 `app.config.json`、生成 `src/app_config.zig`、
调用 `zig build`，然后启动应用。无需手动执行 `zig build`。

常用命令：

```bash
zant run
zant run --watch      # 热更新：改动后自动重编译并重启（重启式）
zant run --release
zant build
zant build --release
zant package
zant sync
```

## 热更新

```bash
zant run --watch
```

监听：

- `src/`（包括 `src/components/` 下的直接文件）
- `src/main.zig`、`src/state.zig`
- `app.config.json`
- `build.zig`、`build.zig.zon`

文件变化后自动重新编译并重启应用。当前是**重启式热更新**：

- 会重新创建窗口和状态
- 编译失败时保留当前窗口，继续等待下一次改动
- App Sandbox 模式暂不支持（需要 LaunchServices 启动，无法可靠重启）

## 打包与签名

```bash
zant package
```

生成 `zig-out/<应用标题>.app`：

- 读取 Bundle ID、版本、窗口配置
- 生成 Info.plist
- 沙盒开启时写入 entitlements
- 使用配置中的 Developer ID 签名
- 无开发者账号时使用 ad-hoc 签名

未自动做 notarization。

## 组件与状态示例

生成项目包含完整的最小示例：

```text
src/state.zig                   共享状态 AppState 与动作
src/components/stat_card.zig    自定义展示组件（props）
src/components/counter_card.zig 自定义子组件（props + 事件回调）
src/components/dashboard.zig    自定义父组件（组合子组件）
src/main.zig                    持有状态并渲染 dashboard
```

数据流：

```text
main 持有 AppState
  → dashboard 读取 state.clicks
  → counter_card 接收 count / on_increment / on_reset
  → 子组件按钮触发回调
  → AppState 修改
  → Runtime 重绘所有读取该状态的组件
```

这是最简单的跨组件状态和事件传递方式。需要全局或异步状态时，再使用
`zant.store.Store(T)`，见 [状态管理](state.md)。

## AI Coding Agent 文件

生成项目还包含：

- `DESIGN.md`：UI/UX 设计指导原则（与框架同一份），**优先级最高**：所有界面与交互的设计、
  实现和评审都必须遵循它，与其他约定冲突时以它为准。
- `AGENTS.md`：开头声明 DESIGN.md 的最高优先级；之后是项目介绍、目录、命令、状态与组件
  规范（通用组件用框架 `ui.*`，应用只写业务组件）、沙盒/签名规则。
- `CLAUDE.md`：Claude Code 入口，同样先声明 DESIGN.md，再引用 `AGENTS.md`。

这些文件让 Codex / Claude Code 在修改项目前先理解约定，减少误改。
