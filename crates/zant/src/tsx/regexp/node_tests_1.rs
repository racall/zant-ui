// Node v24 oracle; UTF-8 and split capture normalization are explicit.
use super::*;

#[test]
fn node_051() {
    // JS: "aba".match(new RegExp("a","gy"))
    check("match", "a", "gy", "aba", "", "[\"a\"]");
}

#[test]
fn node_052() {
    // JS: (()=>{let r=new RegExp("(a)","g");r.lastIndex=0;return [Array.from("aba".matchAll(r),m=>[m[0],m.index]),r.lastIndex]})()
    check("all", "(a)", "g", "aba", "0", "[[[\"a\",0],[\"a\",2]],0]");
}

#[test]
fn node_053() {
    // JS: (()=>{let r=new RegExp("","gu");r.lastIndex=0;return [Array.from("😀".matchAll(r),m=>[m[0],m.index]),r.lastIndex]})()
    check("all", "", "gu", "😀", "0", "[[[\"\",0],[\"\",2]],0]");
}

#[test]
fn node_054() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=2;return [Array.from("aba".matchAll(r),m=>[m[0],m.index]),r.lastIndex]})()
    check("all", "a", "g", "aba", "2", "[[[\"a\",2]],2]");
}

#[test]
fn node_055() {
    // JS: (()=>{let r=new RegExp("a","gy");r.lastIndex=0;return [Array.from("aba".matchAll(r),m=>[m[0],m.index]),r.lastIndex]})()
    check("all", "a", "gy", "aba", "0", "[[[\"a\",0]],0]");
}

#[test]
fn node_056() {
    // JS: "zabq".replace(new RegExp("(a)(b)",""),"$$|$&|$`|$'|$1|$2")
    check(
        "replace",
        "(a)(b)",
        "",
        "zabq",
        "$$|$&|$`|$'|$1|$2",
        "\"z$|ab|z|q|a|bq\"",
    );
}

#[test]
fn node_057() {
    // JS: "a".replace(new RegExp("(a)",""),"$0|$00|$01|$10|$11|$99")
    check(
        "replace",
        "(a)",
        "",
        "a",
        "$0|$00|$01|$10|$11|$99",
        "\"$0|$00|a|a0|a1|$99\"",
    );
}

#[test]
fn node_058() {
    // JS: "a".replace(new RegExp("(a)(b)?",""),"$2-$1")
    check("replace", "(a)(b)?", "", "a", "$2-$1", "\"-a\"");
}

#[test]
fn node_059() {
    // JS: "a".replace(new RegExp("(?<x>a)(?<y>b)?",""),"$<x>/$<y>/$<z>")
    check(
        "replace",
        "(?<x>a)(?<y>b)?",
        "",
        "a",
        "$<x>/$<y>/$<z>",
        "\"a//\"",
    );
}

#[test]
fn node_060() {
    // JS: "a".replace(new RegExp("a",""),"$<x>")
    check("replace", "a", "", "a", "$<x>", "\"$<x>\"");
}

#[test]
fn node_061() {
    // JS: "a".replace(new RegExp("(?<x>a)",""),"$<x")
    check("replace", "(?<x>a)", "", "a", "$<x", "\"$<x\"");
}

#[test]
fn node_062() {
    // JS: "abcdefghij".replace(new RegExp("(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)",""),"$10/$11/$01")
    check(
        "replace",
        "(a)(b)(c)(d)(e)(f)(g)(h)(i)(j)",
        "",
        "abcdefghij",
        "$10/$11/$01",
        "\"j/a1/a\"",
    );
}

#[test]
fn node_063() {
    // JS: "aba".replace(new RegExp("a","g"),"[$&]")
    check("replace", "a", "g", "aba", "[$&]", "\"[a]b[a]\"");
}

#[test]
fn node_064() {
    // JS: "ab".replace(new RegExp("","g"),"-")
    check("replace", "", "g", "ab", "-", "\"-a-b-\"");
}

#[test]
fn node_065() {
    // JS: "😀".replace(new RegExp("","gu"),"-")
    check("replace", "", "gu", "😀", "-", "\"-😀-\"");
}

#[test]
fn node_066() {
    // JS: "ba".replace(new RegExp("a","y"),"x")
    check("replace", "a", "y", "ba", "x", "\"ba\"");
}

#[test]
fn node_067() {
    // JS: "abc".replace(new RegExp("x",""),"z")
    check("replace", "x", "", "abc", "z", "\"abc\"");
}

#[test]
fn node_068() {
    // JS: "😀中中".replace(new RegExp("(中)","g"),"$1!")
    check("replace", "(中)", "g", "😀中中", "$1!", "\"😀中!中!\"");
}

#[test]
fn node_069() {
    // JS: "aa".replace(new RegExp("(a)","g"),"$`|$'")
    check("replace", "(a)", "g", "aa", "$`|$'", "\"|aa|\"");
}

#[test]
fn node_070() {
    // JS: "a".replace(new RegExp("a",""),"$$&")
    check("replace", "a", "", "a", "$$&", "\"$&\"");
}

#[test]
fn node_071() {
    // JS: "a".replace(new RegExp("a",""),"$")
    check("replace", "a", "", "a", "$", "\"$\"");
}

#[test]
fn node_072() {
    // JS: "a".replace(new RegExp("a","g"),"$100")
    check("replace", "a", "g", "a", "$100", "\"$100\"");
}

#[test]
fn node_073() {
    // JS: "a".replace(new RegExp("(?<名>a)",""),"$<名>")
    check("replace", "(?<名>a)", "", "a", "$<名>", "\"a\"");
}

#[test]
fn node_074() {
    // JS: Array.from("".split(new RegExp(",","")),x=>(x??"").toWellFormed())
    check("split", ",", "", "", "", "[\"\"]");
}

#[test]
fn node_075() {
    // JS: Array.from("".split(new RegExp("","")),x=>(x??"").toWellFormed())
    check("split", "", "", "", "", "[]");
}

#[test]
fn node_076() {
    // JS: Array.from("a,b,".split(new RegExp(",","")),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b,", "", "[\"a\",\"b\",\"\"]");
}

#[test]
fn node_077() {
    // JS: Array.from("a,b".split(new RegExp("(,)","")),x=>(x??"").toWellFormed())
    check("split", "(,)", "", "a,b", "", "[\"a\",\",\",\"b\"]");
}

#[test]
fn node_078() {
    // JS: Array.from("a-b".split(new RegExp("(x)?-","")),x=>(x??"").toWellFormed())
    check("split", "(x)?-", "", "a-b", "", "[\"a\",\"\",\"b\"]");
}

#[test]
fn node_079() {
    // JS: Array.from("ab".split(new RegExp("","")),x=>(x??"").toWellFormed())
    check("split", "", "", "ab", "", "[\"a\",\"b\"]");
}

#[test]
fn node_080() {
    // JS: Array.from("😀a".split(new RegExp("","u")),x=>(x??"").toWellFormed())
    check("split", "", "u", "😀a", "", "[\"😀\",\"a\"]");
}

#[test]
fn node_081() {
    // JS: Array.from("😀".split(new RegExp("","")),x=>(x??"").toWellFormed())
    check("split", "", "", "😀", "", "[\"�\",\"�\"]");
}

#[test]
fn node_082() {
    // JS: Array.from("ab".split(new RegExp("a*","")),x=>(x??"").toWellFormed())
    check("split", "a*", "", "ab", "", "[\"\",\"b\"]");
}

#[test]
fn node_083() {
    // JS: Array.from("ab".split(new RegExp("$","")),x=>(x??"").toWellFormed())
    check("split", "$", "", "ab", "", "[\"ab\"]");
}

#[test]
fn node_084() {
    // JS: Array.from("ab".split(new RegExp("^","")),x=>(x??"").toWellFormed())
    check("split", "^", "", "ab", "", "[\"ab\"]");
}

#[test]
fn node_085() {
    // JS: Array.from("ab".split(new RegExp("(?=b)","")),x=>(x??"").toWellFormed())
    check("split", "(?=b)", "", "ab", "", "[\"a\",\"b\"]");
}

#[test]
fn node_086() {
    // JS: Array.from("a,b,c".split(new RegExp(",","y"),2),x=>(x??"").toWellFormed())
    check("split", ",", "y", "a,b,c", "2", "[\"a\",\"b\"]");
}

#[test]
fn node_087() {
    // JS: Array.from("a,b".split(new RegExp("(,)",""),2),x=>(x??"").toWellFormed())
    check("split", "(,)", "", "a,b", "2", "[\"a\",\",\"]");
}

#[test]
fn node_088() {
    // JS: Array.from("a,b".split(new RegExp(",",""),0),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b", "0", "[]");
}

#[test]
fn node_089() {
    // JS: Array.from("a,b".split(new RegExp(",",""),-1),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b", "-1", "[\"a\",\"b\"]");
}

#[test]
fn node_090() {
    // JS: Array.from("a,b".split(new RegExp(",",""),4294967296),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b", "4294967296", "[]");
}

#[test]
fn node_091() {
    // JS: Array.from("a,b".split(new RegExp(",",""),1.9),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b", "1.9", "[\"a\"]");
}

#[test]
fn node_092() {
    // JS: Array.from("a,b".split(new RegExp(",",""),NaN),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b", "NaN", "[]");
}

#[test]
fn node_093() {
    // JS: Array.from("a,b".split(new RegExp(",",""),Infinity),x=>(x??"").toWellFormed())
    check("split", ",", "", "a,b", "Infinity", "[]");
}

#[test]
fn node_094() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=7;return ["ba".search(r),r.lastIndex]})()
    check("search", "a", "g", "ba", "7", "[1,7]");
}

#[test]
fn node_095() {
    // JS: (()=>{let r=new RegExp("a","y");r.lastIndex=7;return ["ba".search(r),r.lastIndex]})()
    check("search", "a", "y", "ba", "7", "[-1,7]");
}

#[test]
fn node_096() {
    // JS: (()=>{let r=new RegExp("z","g");r.lastIndex=3;return ["ba".search(r),r.lastIndex]})()
    check("search", "z", "g", "ba", "3", "[-1,3]");
}

#[test]
fn node_097() {
    // JS: (()=>{let r=new RegExp("中","");r.lastIndex=0;return ["😀中".search(r),r.lastIndex]})()
    check("search", "中", "", "😀中", "0", "[2,0]");
}

#[test]
fn node_098() {
    // JS: (()=>{try{new RegExp("(","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "(",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /(/: Unterminated group\"]",
    );
}

#[test]
fn node_099() {
    // JS: (()=>{try{new RegExp(")","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        ")",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /)/: Unmatched ')'\"]",
    );
}

#[test]
fn node_100() {
    // JS: (()=>{try{new RegExp("[","")}catch(e){return [e.name,e.message]}})()
    check(
        "error",
        "[",
        "",
        "",
        "",
        "[\"SyntaxError\",\"Invalid regular expression: /[/: Unterminated character class\"]",
    );
}
