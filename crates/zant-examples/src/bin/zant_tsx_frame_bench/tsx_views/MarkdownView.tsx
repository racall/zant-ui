// The `<Markdown>` of `tsx/corpus/markdown_bench.tsx` with its source as a
// **prop** instead of a `useState`, for the TSX frame benchmark
// (`zant-tsx-frame-bench`). A driver cannot grow the root
// component's state from outside: codegen keeps root
// state private, so the hand-written benchmark driver owns the streamed text
// and hands it in here. Same widget, same options, same keys
// (`md-<site id>`): only where the text comes from differs.
import { Markdown } from "zant";

export function MarkdownView({ source }: { source: string }) {
  return <Markdown source={source} streaming caret="block" fontSize={15} contentWidth={600} />;
}
