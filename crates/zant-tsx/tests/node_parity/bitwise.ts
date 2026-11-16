// `**`, bitwise and shift operators on numbers (ToInt32 / ToUint32, shift
// counts modulo 32): printed output must match Node.

function flags(read: boolean, write: boolean, run: boolean): number {
  let mode = 0;
  if (read) mode |= 1 << 2;
  if (write) mode |= 1 << 1;
  if (run) mode |= 1;
  return mode;
}

function bits(n: number): string {
  let text = "";
  for (let i = 7; i >= 0; i--) {
    text += (n >> i) & 1 ? "1" : "0";
  }
  return text;
}

export function run(): void {
  const a = 6;
  const b = 3;
  console.log(a & b, a | b, a ^ b, ~a, ~~3.7, ~~-3.7);
  console.log(2 ** 10, 2 ** -1, (-8) ** (1 / 3), 2 ** 3 ** 2, 1 ** NaN);
  console.log(1 << 31, 1 << 32, 1 << 33, -1 >> 28, -1 >>> 28, -1 >>> 0);
  console.log(3.9 | 0, -3.9 | 0, 4294967296 | 0, 2147483648 | 0, NaN | 0, Infinity & 1);
  console.log(flags(true, false, true), flags(false, true, false), bits(165));
  let x = 0xff;
  x &= 0x0f;
  x ^= 3;
  x <<= 4;
  x >>= 1;
  x >>>= 1;
  x **= 2;
  console.log(x, a + b * 2 & 7, (a | b) === 7, a < b === false);
}
