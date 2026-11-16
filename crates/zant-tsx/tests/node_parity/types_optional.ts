// docs/tsx.md:210,214,216: optional operations, truthiness and flow narrowing.
interface Peer { name: string; online: boolean; files: string[] }
interface Profile { title?: string; count?: number; peer?: Peer }
function peerSummary(peer: Peer | null): string {
  const name = peer ? peer.name : "absent";
  const online = peer !== null && peer.online;
  const empty = !peer || peer.files.length === 0;
  return name + ":" + String(online) + ":" + String(empty);
}
function early(peer: Peer | null): string {
  if (!peer) return "none";
  return peer.name.toUpperCase();
}
function capture(peer: Peer | null): string {
  if (peer) {
    const names = [1, 2].map(x => peer.name + String(x));
    return names.join(",");
  }
  return "none";
}
function optionalString(s: string | null): string {
  return (s ? s.trim() : "false") + ":" + (s?.trim() ?? "null") + ":" + (s || "fallback");
}
function optionalNumber(n: number | null): string {
  return (n ? String(n + 1) : "false") + ":" + String(n ?? 99);
}
function optionalBool(b: boolean | null): string {
  return b ? "true:" + String(!b) : "false";
}
function letFlow(start: Peer | null): string {
  let peer: Peer | null = start;
  let text = "";
  if (peer !== null) {
    text += peer.name;
    peer = null;
    text += String(peer === null);
    peer = { name: "again", online: false, files: [] };
    if (peer !== null) text += peer.name;
  }
  return text;
}
function loopFlow(): string {
  let match: string | null = "x";
  let i = 0;
  let text = "";
  while (match !== null) {
    text += match.toUpperCase();
    i += 1;
    match = i < 3 ? "y" : null;
  }
  return text;
}
function forFlow(): string {
  let value: string | null = "a";
  let result = "";
  for (let i = 0; value !== null; i += 1) {
    result += value.toUpperCase();
    value = i < 1 ? "b" : null;
  }
  return result;
}
export function run(): void {
  const p: Peer = { name: "Ada", online: true, files: ["a"] };
  const q: Peer = { name: "", online: false, files: [] };
  console.log("peer", peerSummary(p), peerSummary(q), peerSummary(null));
  console.log("early-capture", early(p), early(null), capture(p), capture(null));
  console.log("string", optionalString(null), optionalString(""), optionalString(" a "));
  console.log("number", optionalNumber(null), optionalNumber(0), optionalNumber(2), optionalNumber(NaN));
  console.log("bool", optionalBool(null), optionalBool(false), optionalBool(true));
  const missing: Profile = {};
  const present: Profile = { title: "  ", count: 0, peer: p };
  console.log("optional", missing.title?.trim() ?? "none", missing.count ?? 9, missing.peer?.name ?? "none");
  console.log("present", present.title?.trim() ?? "none", present.count ?? 9, present.peer?.name ?? "none");
  console.log("and", (missing.peer && missing.peer.name) ?? "empty");
  console.log("let", letFlow(p), letFlow(null), loopFlow(), forFlow());
  const nums: number[] = [0, 1, -1, NaN];
  for (const n of nums) console.log("truth-number", String(n), !!n, n || 7);
  const words: string[] = ["", "0", "false"];
  for (const s of words) console.log("truth-string", s, !!s, s || "fallback");
  const empty: number[] = [];
  console.log("truth-containers", !!empty, !!p, !false, !missing.title, !missing.peer);
  console.log("non-null-assertion", present.peer!.name, present.title!.length);
}
