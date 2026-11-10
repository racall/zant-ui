# __APP_TITLE__

用 TypeScript + TSX 编写的原生桌面应用，应用标识 `__BUNDLE_ID__`。
TSX 在编译期转成 Zig，无 JS 运行时，无需 npm 依赖。

```bash
zant run
zant run --watch      # 修改 src/ 后重新编译并重启，状态会重建
zant compile          # 只编译
zant package          # 本机测试包
zant build            # 发布构建
zant sync             # 从 app.config.json 重新生成配置
```

- `src/App.tsx`：根组件，持有输入框和计数状态。
- `src/components/`：Dashboard / StatCard / CounterCard，通过 props 和回调共享状态。
- `native/`：原生模块（Zig）。`system.ts` 是接口规范，`system.zig` 是实现，`lib.zig` 列出全部模块；
  TSX 用 `import { info } from "native/system"` 直接调用，类型不一致时编译报错。
  `zig build test` 运行原生模块的单元测试。
- `app.config.json`：标题、窗口、应用标识；构建时生成配置，入口也由工具链生成，不用写 `main.zig`。
- `zant.d.ts` / `tsconfig.json`：编辑器类型与补全；本机有 TypeScript 时用 `tsc --noEmit -p .` 检查。
- `assets/icons/`：替换 PNG 即可修改图标，SVG 是可编辑源文件。
- `build.zig`：`addTsxApp` 编译 src/ 下的 TSX 与 native/ 下的原生模块，`configureApp` 处理桌面资源。

语法与 hooks：[TSX 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx.md)；
原生模块：[native 文档](https://github.com/racall/zant-ui/blob/main/docs/tsx-native.md)。
发布：[构建文档](https://github.com/racall/zant-ui/blob/main/docs/build.md)。
本地框架依赖的相对路径在 `build.zig.zon`。
