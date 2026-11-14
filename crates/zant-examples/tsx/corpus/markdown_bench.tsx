import { useState, Markdown } from "zant";
export default function MarkdownBench() {
  const [source, setSource] = useState("");
  return <Markdown source={source} streaming caret="block" fontSize={15} contentWidth={600} />;
}
