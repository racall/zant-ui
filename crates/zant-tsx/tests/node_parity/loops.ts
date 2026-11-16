// `for…in` (own enumerable keys, in insertion order): printed output must
// match Node.

function total(scores: Record<string, number>): number {
  let sum = 0;
  for (const name in scores) {
    sum += scores[name] ?? 0;
  }
  return sum;
}

function keys(scores: Record<string, number>): string {
  let names = "";
  for (let name in scores) names += name + ";";
  return names;
}

function firstAbove(scores: Record<string, number>, limit: number): string {
  for (const name in scores) {
    if ((scores[name] ?? 0) > limit) {
      return name;
    }
  }
  return "none";
}

export function run(): void {
  const scores: Record<string, number> = { ada: 3, bob: 5, cy: 1 };
  console.log(total(scores));
  console.log(keys(scores));
  console.log(firstAbove(scores, 4));
  console.log(firstAbove(scores, 9));
  console.log(keys({}));
}
