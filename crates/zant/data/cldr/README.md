# CLDR data

`plurals.json` is the cardinal plural rules file from the Unicode CLDR JSON
distribution (`cldr-json/cldr-core/supplemental/plurals.json`, CLDR 49), used
unchanged by `zant::i18n::plural` (rules are parsed and evaluated per UTS #35;
each rule's `@integer` / `@decimal` samples are the test vectors).
`LICENSE` is the Unicode License v3 it is distributed under.

Update: replace `plurals.json` with the newer release's file and run
`cargo test -p zant i18n::plural`.
