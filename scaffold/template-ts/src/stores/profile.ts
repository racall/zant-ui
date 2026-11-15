import { createStore } from "zant";

interface Profile {
  displayName: string;
  accent: boolean;
}

// 跨组件共享状态：各组件直接读取或更新 store，无需通过 props 传递。
export const profile = createStore<Profile>({ displayName: "开发者", accent: false });
