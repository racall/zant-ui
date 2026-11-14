# Zig ↔ Rust 对照工具（WP-0.4）

Zig 版是行为基准，Rust 版必须逐像素、逐坐标一致。两边使用同一套 Skia 桥，
所以布局应当完全相同、像素几乎相同。

```bash
# 抓一次：<zig|rust> <二进制> <输出目录> [light|dark] [宽x高]
scripts/parity/capture.sh zig zant-demo /tmp/demo-zig light 900x640

# 全套对照：浅/深色 × 三种窗口尺寸，逐项比对 layout.txt 与 shot.png
scripts/parity/compare.sh zant-demo
scripts/parity/compare.sh zant-components
```

- `capture.sh` 设置 `ZANT_SNAPSHOT` / `ZANT_SNAPSHOT_EXIT` / `ZANT_LAYOUT_FILE` /
  `ZANT_NO_MOTION` / `ZANT_THEME`（`ZANT_DARK=1` 用于 Zig 示例）/ `ZANT_WINDOW_SIZE`，
  Zig 二进制取自 `zig-out-baseline/bin/`，Rust 取自 `target/release/`。
- `layoutdiff.py`：按 key 对齐，key 集合必须相同，坐标差 ≤ 0.01。
- `imgdiff.py`：纯标准库 PNG 解码/比较；任一通道差 > 2 的像素占比 ≤ 0.1% 算一致，
  不一致时用 `--diff` 输出差异图。**不需要 Pillow。**
- 交互对照复用现有脚本：`scripts/mac/ui-scenario.swift`、
  `scripts/win/{ui-auto,components-check,pdf-check,resize-check,stream-resize-check}.ps1`
  （把可执行文件路径改成参数）。Windows 的窗口必须在已登录桌面里运行。

注意：`ZANT_WINDOW_SIZE` 是迁移期间给 Zig `platform.createWindow` 加的自动化钩子
（`docs/rust-port/status.md` 的 Zig 变更日志有登记），只影响窗口初始尺寸。
