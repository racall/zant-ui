import { createStore } from "zant";

interface Profile {
  displayName: string;
  accent: boolean;
}

// State shared across components: each reads or updates the store directly, no props needed.
export const profile = createStore<Profile>({ displayName: "Developer", accent: false });
