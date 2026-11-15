# Zant Markdown 渲染一览

**粗体**、*斜体*、~~删除线~~、==高亮==、`行内代码`、[链接](https://www.rust-lang.org)与脚注[^1]。
行内公式 $e^{i\pi}+1=0$；金额 $20 与 $10 保持普通文本。

> [!TIP]
> 提示块支持 NOTE / TIP / IMPORTANT / WARNING / CAUTION。

## 列表与表格

- 无序列表
  - 嵌套内容
- [x] 已完成
- [ ] 待办

| 内容 | 实现 |
|:--|--:|
| Markdown | 原生绘制 |
| 公式 | STIX Two Math |

## 公式与化学式

$$
\int_{-\infty}^{\infty}e^{-x^2}\,dx=\sqrt{\pi},\qquad
A=\begin{pmatrix}a&b\\c&d\end{pmatrix}
$$

$$
\ce{N2 + 3H2 <=> 2NH3}\qquad\pu{123 kJ mol-1}
$$

## 代码与图表

```rust
fn main() {
    println!("hello, zant");
}
```

```mermaid
flowchart LR
  A[Markdown] --> B[remend]
  B --> C[块缓存]
  C --> D[原生绘制]
```

[^1]: 脚注集中显示在文末，支持返回引用位置。
