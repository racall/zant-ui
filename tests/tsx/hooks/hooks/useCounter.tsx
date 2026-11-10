// A hook in a subdirectory: its imports are relative to it.
import { useState } from "zant";
import { step } from "../config";

export function useCounter(start: number) {
  const [count, setCount] = useState(start);
  return { count, increment: () => setCount(count + step), reset: () => setCount(start) };
}
