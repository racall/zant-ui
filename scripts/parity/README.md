# Zig ↔ Rust 对照工具（迁移期，WP-0.4）

Zig 版曾是行为基准，Rust 版必须逐像素、逐坐标一致（迁移记录见
`docs/rust-port/status.md`）。Zig 工具链在 M8 已删除；这套脚本需要旧的
Zig 基线产物（`zig-out-baseline/bin/`，用 `ZANT_ZIG_BIN` 指向它），否则只能
抓 Rust 一侧的快照。帧基准 `bench.sh` 在没有 Zig 基线时改用
`docs/rust-port/status.md` 记录的基线数字（见脚本头部说明）。

```bash
# 抓一次：<zig|rust> <二进制> <输出目录> [light|dark] [宽x高]
scripts/parity/capture.sh zig zant-demo /tmp/demo-zig light 900x640

# 全套对照：浅/深色 × 三种窗口尺寸，逐项比对 layout.txt 与 shot.png
scripts/parity/compare.sh zant-demo
scripts/parity/compare.sh zant-components
```

- `capture.sh` 设置 `ZANT_SNAPSHOT` / `ZANT_SNAPSHOT_EXIT` / `ZANT_LAYOUT_FILE` /
  `ZANT_NO_MOTION` / `ZANT_THEME`（`ZANT_DARK=1` 用于旧的 Zig 示例）/ `ZANT_WINDOW_SIZE`，
  Zig 二进制取自 `zig-out-baseline/bin/`（迁移期产物），Rust 取自 `target/release/`。
- `layoutdiff.py`：按 key 对齐，key 集合必须相同，坐标差 ≤ 0.01。
- `imgdiff.py`：纯标准库 PNG 解码/比较；任一通道差 > 2 的像素占比 ≤ 0.1% 算一致，
  不一致时用 `--diff` 输出差异图。**不需要 Pillow。**
- 交互对照复用现有脚本：`scripts/mac/ui-scenario.swift`、
  `scripts/win/{ui-auto,components-check,pdf-check,resize-check,stream-resize-check}.ps1`
  （把可执行文件路径改成参数）。Windows 的窗口必须在已登录桌面里运行。

注意：`ZANT_WINDOW_SIZE` 是迁移期间给 Zig `platform.createWindow` 加的自动化钩子
（Rust 版照搬；`docs/rust-port/status.md` 的 Zig 变更日志有登记），只影响窗口初始尺寸。
