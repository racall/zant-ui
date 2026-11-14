import { View, Text } from "zant";

// A second project module (not used by `App.tsx`): `zant-build` declares it in
// the generated `main.rs`, so `src/main.rs` sees it as `crate::greeting`.
//
// The fixture works around three `zant-tsx` code-generation gaps
// (`zant-build` itself is not involved) so the end-to-end
// build-script check can compile:
// - non-`Copy` prop reads (`self.props.label` in a `<Text>` child) need a
//   `.clone()`, so the prop is not rendered here;
// - static text children emit `ui::text("…".into(), …)`, which is ambiguous
//   for `impl Into<Str>`, so the text is interpolated;
// - non-stateful child builders drop their `props` argument (the fixture does
//   not call `Greeting`).
//
// The missing `use zant::nodes;` of the module preamble is handled by
// `#[macro_use] extern crate zant;` in `src/main.rs`.

export interface GreetingProps {
  label: string;
}

export function Greeting(_props: GreetingProps) {
  return (
    <View padding={8}>
      <Text>{`greeting module #${1}`}</Text>
    </View>
  );
}
