// try / catch / finally / throw: printed output must match Node.

function check(n: number): number {
  if (n < 0) {
    throw new RangeError("negative: " + n);
  }
  return n * 2;
}

function viaCheck(n: number): number {
  return check(n) + 1;
}

function safe(n: number): number {
  try {
    return viaCheck(n);
  } catch (e) {
    return -1;
  }
}

function describe(n: number): string {
  try {
    const v = check(n);
    return "ok " + v;
  } catch (e) {
    if (e instanceof RangeError) {
      return "range " + e.message;
    }
    if (e instanceof Error) {
      return "error " + e.message;
    }
    return "other " + String(e);
  }
}

function throwsString(): number {
  throw "busy";
}

function throwsNumber(): number {
  throw 42;
}

function kinds(which: number): string {
  try {
    if (which === 0) {
      throwsString();
    }
    if (which === 1) {
      throwsNumber();
    }
    if (which === 2) {
      throw new TypeError("bad type");
    }
    return "none";
  } catch (e) {
    if (typeof e === "string") {
      return "string:" + e;
    }
    if (typeof e === "number") {
      return "number:" + (e + 1);
    }
    return `${e} / ${typeof e}`;
  }
}

function order(): string {
  let log = "";
  try {
    log += "a";
    throw new Error("x");
  } catch (e) {
    log += "b";
  } finally {
    log += "c";
  }
  return log;
}

function finallyRuns(): number {
  try {
    return 1;
  } finally {
    console.log("finally runs");
  }
}

function finallyReturn(): number {
  try {
    throw new Error("lost");
  } finally {
    return 7;
  }
}

function loopJumps(): string {
  let out = "";
  for (let i = 0; i < 5; i++) {
    try {
      if (i === 1) {
        continue;
      }
      if (i === 3) {
        break;
      }
      out += i;
    } finally {
      out += "f";
    }
  }
  return out;
}

function rethrow(n: number): number {
  try {
    return check(n);
  } catch (e) {
    throw e;
  }
}

function nested(): string {
  try {
    try {
      throw new Error("inner");
    } finally {
      console.log("inner finally");
    }
  } catch (e) {
    return "outer caught " + (e instanceof Error ? e.message : "?");
  }
}

function validate(s: string): void {
  if (s.length === 0) {
    throw new Error("empty");
  }
}

function tryValidate(s: string): string {
  try {
    validate(s);
    return "valid";
  } catch (e) {
    return "invalid";
  }
}

function catchWithoutBinding(): string {
  try {
    check(-1);
    return "no";
  } catch {
    return "caught";
  }
}

export function run(): void {
  console.log(safe(2), safe(-1));
  console.log(describe(3));
  console.log(describe(-2));
  console.log(kinds(0));
  console.log(kinds(1));
  console.log(kinds(2));
  console.log(kinds(3));
  console.log(order());
  console.log(finallyRuns());
  console.log(finallyReturn());
  console.log(loopJumps());
  console.log(rethrow(5));
  try {
    rethrow(-5);
  } catch (e) {
    console.log("rethrown", String(e));
  }
  console.log(nested());
  console.log(tryValidate(""), tryValidate("x"));
  console.log(catchWithoutBinding());
  const error = new Error("plain");
  console.log(error.name, error.message, String(error));
}
