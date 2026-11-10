// native/system.zig 的规范：TSX 按这里的类型调用，编译时检查 Zig 实现是否一致。
import type { u32 } from "zant";

export type Os = "macos" | "windows" | "other";

export interface SystemInfo {
  os: Os;
  cpus: u32;
  zig: string;
}

export declare function info(): SystemInfo;
