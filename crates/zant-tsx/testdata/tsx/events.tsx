import { useState, View, Text, Button, Input, Switch, Checkbox } from "zant";
import type { u16 as Small } from "zant";

export default function Events() {
  const [count, setCount] = useState<u32>(0);
  const [name, setName] = useState<string>("Zant");
  const [on, setOn] = useState<boolean>(false);
  const ui = count + 1;
  const self = "render local";
  const unused = 7;
  const small: Small = 2;
  const computed: u32 = 1 + 2;
  const choice = count + (on ? 1 : 2);
  const otherChoice = (on ? 1 : 2) + count;
  const narrow: f32 = count;
  const wide: number = narrow;
  const back: f32 = wide;
  const mixed = count + 0.5;
  const fractionalChoice = count + (on ? 1.25 : 2);
  const peer = 0.1234567890123 + (narrow + wide);
  const type = "escaped keyword";
  const u8 = "escaped primitive";

  function twice(count: u32): u32 { return count * 2; }
  function std() { console.log("method may use a module name"); }
  function zant() { console.log("zant method"); }
  function tsx() { console.log("tsx method"); }
  function ignore(count: u32) { setCount(0); }
  function nothing(self: string, ui: boolean) { console.log("unused params"); }
  function increment() {
    setCount(count + 1);
    setCount(count + 1);
  }
  function save(value: string) {
    setName(value);
    console.log(name, value, count, on);
  }
  function toggle(checked) { setOn(checked); }
  function _onClick1() { console.log("user method wins"); }
  function _setName() { console.log("adapter must be renamed"); }
  function arithmetic() {
    let local: u32 = count;
    local += 2;
    local++;
    local -= 1;
    local *= 2;
    local /= 2;
    local %= 10;
    local--;
    if (local > 0) { setCount(local); } else { setCount(0); }
    increment();
    ignore(count);
    nothing("unused", false);
    std();
    zant();
    tsx();
    let writeOnly = 1;
    writeOnly = 2;
    let unchanged = "const in Rust";
    console.log(unchanged);
  }
  const reset = () => setCount(0);
  const format = (n: u32): string => `value=${n}`;
  return (
    <View>
      <Text numberOfLines={on ? 1 : 2}>{twice(ui)} {self} {format(count)} {small} {computed} {choice} {otherChoice} {back} {mixed} {fractionalChoice} {peer} {type} {u8}</Text>
      <Button onClick={() => { setCount(p => p + 1); setName(p => p + "!"); }}>Functional</Button>
      <Button onClick={increment}>Snapshot</Button>
      <Button onClick={arithmetic}>Arithmetic</Button>
      <Button onClick={reset}>Reset</Button>
      <Button onClick={_onClick1}>User method</Button>
      <Button onClick={_setName}>Another method</Button>
      <Input value={name} onChange={setName} onSubmit={reset} />
      <Input onChange={save} />
      <Input onChange={() => console.log("ignored parameter")} />
      <Switch checked={on} onChange={setOn} />
      <Switch onChange={toggle} />
      <Checkbox onChange={(on: boolean) => { const ui = on; setOn(ui); }}>Inline</Checkbox>
    </View>
  );
}
