// Numeric built-ins documented in docs/tsx.md; deterministic, no clocks/random.
function numeric(text: string): void {
  const n = Number(text);
  console.log("Number", JSON.stringify(text), String(n), String(1 / n));
}

export function run(): void {
  const rounds: number[] = [-2.5, -1.5, -0.5, -0.49999999999999994, -0, 0, 0.49999999999999994, 0.5, 1.5, 2.5, 4503599627370495.5, NaN, Infinity, -Infinity];
  for (const x of rounds) {
    console.log("round", String(x), String(Math.round(x)), String(1 / Math.round(x)));
    console.log("integral", String(Math.floor(x)), String(Math.ceil(x)), String(Math.trunc(x)), String(Math.abs(x)), String(Math.sign(x)));
    console.log("zero-signs", String(1 / Math.ceil(x)), String(1 / Math.trunc(x)), String(1 / Math.sign(x)));
  }
  console.log("empty-math", Math.min(), Math.max(), Math.hypot());
  console.log("extrema-zero", 1 / Math.min(0, -0), 1 / Math.min(-0, 0), 1 / Math.max(0, -0), 1 / Math.max(-0, 0));
  console.log("extrema-nan", Math.min(1, NaN, -1), Math.max(NaN, Infinity));
  console.log("pow", Math.pow(NaN, 0), Math.pow(1, Infinity), Math.pow(-1, Infinity), Math.pow(-0, -3));
  console.log("roots", String(Math.sqrt(-0)), 1 / Math.sqrt(-0), Math.sqrt(-1), Math.cbrt(-8));
  console.log("hypot", Math.hypot(3, 4), Math.hypot(Infinity, NaN), Math.hypot(NaN, Infinity));
  console.log("logs", Math.exp(0), Math.log(1), Math.log2(8), Math.log10(1000), Math.log(0), Math.log(-1));
  console.log("trig", Math.sin(0), Math.cos(0), Math.tan(0), Math.atan(Infinity), Math.atan2(-0, -0));
  console.log("constants", Math.PI, Math.E, Number.EPSILON, Number.MAX_SAFE_INTEGER);
  const fixed: number[] = [0.5, -0.5, 1.005, -1.005, 0.995, 1.25, -1.25, 2.55, 2.675, -0, -0.0001, 1e20, 1e21, -1e21, NaN, Infinity, -Infinity];
  for (const x of fixed) {
    console.log("fixed", String(x), x.toFixed(), x.toFixed(1), x.toFixed(2), x.toFixed(20));
  }
  console.log("fixed-fraction", (1.25).toFixed(1.9), (-0.5).toFixed(-0.9));
  const texts: string[] = ["", " ", "\t\n\r", "\ufeff\u00a0\u2028", "0x10", "0Xff", "0b101", "0B11", "0o17", "0O10", "+0x10", "-0b1", "0x", "0b2", "08", "1e3", "1e", ".5", "1.", "Infinity", "+Infinity", "-Infinity", "inf", "NaN", "12px", "1_000", "-0", "-0.0", "\u0085", "0x20000000000001"];
  for (const text of texts) numeric(text);
  console.log("Number-values", Number(true), Number(false), String(Number(-0)), 1 / Number(-0));
  console.log("predicates", Number.isNaN(NaN), Number.isFinite(Infinity), Number.isInteger(-0), Number.isInteger(1.5));
  console.log("predicate-types", Number.isNaN("NaN"), Number.isFinite("1"), Number.isInteger(true));
  const ints: string[] = ["1012", "0x10", "-0x1f!", "+0Xff", "z!", "  -0", "1e3", "0b11", "", "xyz"];
  for (const text of ints) {
    console.log("parseInt", JSON.stringify(text), String(parseInt(text)), String(parseInt(text, 2)), String(parseInt(text, 16)), String(parseInt(text, 36)));
  }
  console.log("radix", parseInt("12", 0), parseInt("12", 1), parseInt("12", 37), parseInt("12", 16.9), parseInt("12", 4294967306));
  console.log("parseInt-zero", 1 / parseInt("-0"), 1 / parseInt("-0x0", 16));
  const floats: string[] = ["1.25e2tail", "1e", "1e+", "1e-2x", ".5E+3!", "0x10", "-Infinity!", "+Infinity", "inf", "\ufeff -0.0e5x", "1e309", "1e-400", "garbage"];
  for (const text of floats) console.log("parseFloat", JSON.stringify(text), String(parseFloat(text)), String(1 / parseFloat(text)));
  const spellings: number[] = [1e-7, 1e-6, 1e-5, 1e20, 1e21, 1e22, 1000000000000000128, 0.1 + 0.2, Number.MAX_VALUE, Number.MIN_VALUE, Number.MIN_SAFE_INTEGER, -0, NaN, Infinity];
  for (const x of spellings) console.log("String", String(x), x.toString());
  console.log("String-bool", String(true), String(false), String(""));
}
