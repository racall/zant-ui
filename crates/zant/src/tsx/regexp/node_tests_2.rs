// Node v24 oracle; UTF-8 and split capture normalization are explicit.
use super::*;

#[test]
fn node_101() {
    // JS: (()=>{try{new RegExp("[z-a]","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "[z-a]",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /[z-a]/: Range out of order in character class\"]",
    );
}

#[test]
fn node_102() {
    // JS: (()=>{try{new RegExp("*","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "*",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /*/: Nothing to repeat\"]",
    );
}

#[test]
fn node_103() {
    // JS: (()=>{try{new RegExp("a{2,1}","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "a{2,1}",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /a{2,1}/: numbers out of order in {} quantifier\"]",
    );
}

#[test]
fn node_104() {
    // JS: (()=>{try{new RegExp("\\","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "\\",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /\\\\/: \\\\ at end of pattern\"]",
    );
}

#[test]
fn node_105() {
    // JS: (()=>{try{new RegExp("(?x)","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "(?x)",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /(?x)/: Invalid group\"]",
    );
}

#[test]
fn node_106() {
    // JS: (()=>{try{new RegExp("(?<a>x)(?<a>y)","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "(?<a>x)(?<a>y)",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /(?<a>x)(?<a>y)/: Duplicate capture group name\"]",
    );
}

#[test]
fn node_107() {
    // JS: (()=>{try{new RegExp("\\k<a>","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "\\k<a>",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /\\\\k<a>/u: Invalid named capture referenced\"]",
    );
}

#[test]
fn node_108() {
    // JS: (()=>{try{new RegExp("\\p{Nope}","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "\\p{Nope}",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /\\\\p{Nope}/u: Invalid property name\"]",
    );
}

#[test]
fn node_109() {
    // JS: (()=>{try{new RegExp("\\xZ","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "\\xZ",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /\\\\xZ/u: Invalid escape\"]",
    );
}

#[test]
fn node_110() {
    // JS: (()=>{try{new RegExp("a","gg")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "a",
        "gg",
        "",
        "",
        "[\"SyntaxError\",\"Invalid flags supplied to RegExp constructor 'gg'\"]",
    );
}

#[test]
fn node_111() {
    // JS: (()=>{try{new RegExp("a","uv")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "a",
        "uv",
        "",
        "",
        "[\"SyntaxError\",\"Invalid flags supplied to RegExp constructor 'uv'\"]",
    );
}

#[test]
fn node_112() {
    // JS: (()=>{try{new RegExp("a","z")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "a",
        "z",
        "",
        "",
        "[\"SyntaxError\",\"Invalid flags supplied to RegExp constructor 'z'\"]",
    );
}

#[test]
fn node_113() {
    // JS: (()=>{try{"a".matchAll(new RegExp("a",""))}catch(e){return [e.name,e.message]}})()
    check(
        "all_error",
        "a",
        "",
        "a",
        "",
        "[\"TypeError\",\"String.prototype.matchAll called with a non-global RegExp argument\"]",
    );
}

#[test]
fn node_114() {
    // JS: (()=>{try{"a".replaceAll(new RegExp("a",""),"b")}catch(e){return [e.name,e.message]}})()
    check(
        "replace_error",
        "a",
        "",
        "a",
        "b",
        "[\"TypeError\",\"String.prototype.replaceAll called with a non-global RegExp argument\"]",
    );
}

#[test]
fn node_115() {
    // JS: "aba".replace(new RegExp("(a)","g"),(m,p,offset)=>p+offset)
    check("callback", "(a)", "g", "aba", "", "\"a0ba2\"");
}

#[test]
fn node_116() {
    // JS: "aba".replaceAll(new RegExp("a","g"),"x")
    check("replace_all", "a", "g", "aba", "x", "\"xbx\"");
}

#[test]
fn node_117() {
    // JS: "aba".replaceAll(new RegExp("a","g"),(m,offset)=>String(offset))
    check("callback_all", "a", "g", "aba", "", "\"0b2\"");
}

#[test]
fn node_118() {
    // JS: new RegExp("\\\n","").toString()
    check("source", "\\\n", "", "", "", "\"/\\\\n/\"");
}

#[test]
fn node_119() {
    // JS: new RegExp("\\\r","").toString()
    check("source", "\\\r", "", "", "", "\"/\\\\r/\"");
}

#[test]
fn node_120() {
    // JS: new RegExp("\\\u2028","").toString()
    check("source", "\\ ", "", "", "", "\"/\\\\u2028/\"");
}

#[test]
fn node_121() {
    // JS: new RegExp("\\\\/","").toString()
    check("source", "\\\\/", "", "", "", "\"/\\\\\\\\\\\\//\"");
}

#[test]
fn node_122() {
    // JS: (()=>{try{new RegExp("a**","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "a**",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /a**/: Nothing to repeat\"]",
    );
}

#[test]
fn node_123() {
    // JS: (()=>{try{new RegExp("{","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "{",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /{/u: Lone quantifier brackets\"]",
    );
}

#[test]
fn node_124() {
    // JS: (()=>{try{new RegExp("}","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "}",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /}/u: Lone quantifier brackets\"]",
    );
}

#[test]
fn node_125() {
    // JS: (()=>{try{new RegExp("]","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "]",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /]/u: Lone quantifier brackets\"]",
    );
}

#[test]
fn node_126() {
    // JS: (()=>{try{new RegExp("a{","u")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "a{",
        "u",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /a{/u: Incomplete quantifier\"]",
    );
}

#[test]
fn node_127() {
    // JS: (()=>{try{new RegExp("(?<>","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "(?<>",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /(?<>/: Invalid capture group name\"]",
    );
}

#[test]
fn node_128() {
    // JS: (()=>{let m=new RegExp("(?<x>a)|(?<x>b)","").exec("b");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(?<x>a)|(?<x>b)",
        "",
        "b",
        "",
        "[[\"b\",null,\"b\"],0,\"b\",{\"x\":\"b\"}]",
    );
}

#[test]
fn node_129() {
    // JS: "b".replace(new RegExp("(?<x>a)|(?<x>b)",""),"$<x>")
    check("replace", "(?<x>a)|(?<x>b)", "", "b", "$<x>", "\"b\"");
}
