// Arithmetic on counts (`.length` is a Rust usize): JavaScript numbers are
// signed and divide exactly. Printed output must match Node.

function share(done: number[], all: number[]): number {
  return Math.round((done.length / all.length) * 100);
}

export function run(): void {
  const items = [1, 2, 3];
  const done = items.filter((x) => x > 2);
  const none: number[] = [];
  console.log(done.length / items.length, share(done, items), items.length % 2, items.length - 5);
  const half: number = items.length / 2;
  console.log(half, none.length - 1, none.length / items.length, items.length * 2 - 7);
}
