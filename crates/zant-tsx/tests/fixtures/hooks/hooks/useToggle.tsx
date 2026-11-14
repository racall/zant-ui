// Hooks calling hooks; an effect inside a hook.
import { useState, useEffect } from "zant";

function useFlag(initial: boolean) {
  const [on, setOn] = useState(initial);
  return [on, setOn];
}

export function useToggle() {
  const [on, setOn] = useFlag(false);
  const [flips, setFlips] = useState(0);
  useEffect(() => {
    if (on) setFlips(flips + 1);
  }, [on]);
  return [on, () => setOn(!on), flips];
}
