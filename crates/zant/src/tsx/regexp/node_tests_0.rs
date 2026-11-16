// Node v24 oracle; UTF-8 and split capture normalization are explicit.
use super::*;

#[test]
fn node_001() {
    // JS: (()=>{let m=new RegExp("a","").exec("ba");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "a", "", "ba", "", "[[\"a\"],1,\"ba\",null]");
}

#[test]
fn node_002() {
    // JS: (()=>{let m=new RegExp("a","").exec("x");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "a", "", "x", "", "null");
}

#[test]
fn node_003() {
    // JS: (()=>{let m=new RegExp("(a)(b)?","").exec("a");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(a)(b)?",
        "",
        "a",
        "",
        "[[\"a\",\"a\",null],0,\"a\",null]",
    );
}

#[test]
fn node_004() {
    // JS: (()=>{let m=new RegExp("(?<x>a)(?<y>b)?","").exec("a");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(?<x>a)(?<y>b)?",
        "",
        "a",
        "",
        "[[\"a\",\"a\",null],0,\"a\",{\"x\":\"a\",\"y\":null}]",
    );
}

#[test]
fn node_005() {
    // JS: (()=>{let m=new RegExp("(a)\\1","").exec("zaa");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(a)\\1",
        "",
        "zaa",
        "",
        "[[\"aa\",\"a\"],1,\"zaa\",null]",
    );
}

#[test]
fn node_006() {
    // JS: (()=>{let m=new RegExp("(?<x>a)\\k<x>","").exec("aa");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(?<x>a)\\k<x>",
        "",
        "aa",
        "",
        "[[\"aa\",\"a\"],0,\"aa\",{\"x\":\"a\"}]",
    );
}

#[test]
fn node_007() {
    // JS: (()=>{let m=new RegExp("(?<=a)b","").exec("ab");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "(?<=a)b", "", "ab", "", "[[\"b\"],1,\"ab\",null]");
}

#[test]
fn node_008() {
    // JS: (()=>{let m=new RegExp("(?<!a)b","").exec("cb");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "(?<!a)b", "", "cb", "", "[[\"b\"],1,\"cb\",null]");
}

#[test]
fn node_009() {
    // JS: (()=>{let m=new RegExp("b","").exec("😀中b");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "b", "", "😀中b", "", "[[\"b\"],3,\"😀中b\",null]");
}

#[test]
fn node_010() {
    // JS: (()=>{let m=new RegExp(".","u").exec("😀");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", ".", "u", "😀", "", "[[\"😀\"],0,\"😀\",null]");
}

#[test]
fn node_011() {
    // JS: (()=>{let m=new RegExp(".","").exec("😀");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", ".", "", "😀", "", "[[\"�\"],0,\"😀\",null]");
}

#[test]
fn node_012() {
    // JS: (()=>{let m=new RegExp("😀","").exec("x😀");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "😀", "", "x😀", "", "[[\"😀\"],1,\"x😀\",null]");
}

#[test]
fn node_013() {
    // JS: (()=>{let m=new RegExp("😀+","").exec("😀😀");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "😀+", "", "😀😀", "", "[[\"😀\"],0,\"😀😀\",null]");
}

#[test]
fn node_014() {
    // JS: (()=>{let m=new RegExp("^b","m").exec("a\nb");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "^b", "m", "a\nb", "", "[[\"b\"],2,\"a\\nb\",null]");
}

#[test]
fn node_015() {
    // JS: (()=>{let m=new RegExp(".","s").exec("\n");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", ".", "s", "\n", "", "[[\"\\n\"],0,\"\\n\",null]");
}

#[test]
fn node_016() {
    // JS: (()=>{let m=new RegExp(".","").exec("\n");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", ".", "", "\n", "", "null");
}

#[test]
fn node_017() {
    // JS: (()=>{let m=new RegExp("k","iu").exec("K");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "k", "iu", "K", "", "[[\"K\"],0,\"K\",null]");
}

#[test]
fn node_018() {
    // JS: (()=>{let m=new RegExp("k","i").exec("K");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "k", "i", "K", "", "null");
}

#[test]
fn node_019() {
    // JS: (()=>{let m=new RegExp("s","iu").exec("ſ");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "s", "iu", "ſ", "", "[[\"ſ\"],0,\"ſ\",null]");
}

#[test]
fn node_020() {
    // JS: (()=>{let m=new RegExp("\\p{Script=Han}+","u").exec("a中文");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "\\p{Script=Han}+",
        "u",
        "a中文",
        "",
        "[[\"中文\"],1,\"a中文\",null]",
    );
}

#[test]
fn node_021() {
    // JS: (()=>{let m=new RegExp("(?=(a))","").exec("a");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(?=(a))",
        "",
        "a",
        "",
        "[[\"\",\"a\"],0,\"a\",null]",
    );
}

#[test]
fn node_022() {
    // JS: (()=>{let m=new RegExp("(a)|(b)","").exec("b");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(a)|(b)",
        "",
        "b",
        "",
        "[[\"b\",null,\"b\"],0,\"b\",null]",
    );
}

#[test]
fn node_023() {
    // JS: (()=>{let m=new RegExp("[[a-z]&&[^aeiou]]+","v").exec("aeBCDxyz");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "[[a-z]&&[^aeiou]]+",
        "v",
        "aeBCDxyz",
        "",
        "[[\"xyz\"],5,\"aeBCDxyz\",null]",
    );
}

#[test]
fn node_024() {
    // JS: (()=>{let m=new RegExp(".","v").exec("😀");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", ".", "v", "😀", "", "[[\"😀\"],0,\"😀\",null]");
}

#[test]
fn node_025() {
    // JS: (()=>{let m=new RegExp("[\\q{ab|cd}]","v").exec("cd");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "[\\q{ab|cd}]",
        "v",
        "cd",
        "",
        "[[\"cd\"],0,\"cd\",null]",
    );
}

#[test]
fn node_026() {
    // JS: (()=>{let m=new RegExp("(a)+","").exec("aaa");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check(
        "exec",
        "(a)+",
        "",
        "aaa",
        "",
        "[[\"aaa\",\"a\"],0,\"aaa\",null]",
    );
}

#[test]
fn node_027() {
    // JS: (()=>{let m=new RegExp("a+?","").exec("aaa");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "a+?", "", "aaa", "", "[[\"a\"],0,\"aaa\",null]");
}

#[test]
fn node_028() {
    // JS: (()=>{let m=new RegExp("a$","").exec("a\n");return m&&[Array.from(m,x=>x??null),m.index,m.input,m.groups?Object.fromEntries(Object.entries(m.groups).map(([k,v])=>[k,v??null])):null]})()
    check("exec", "a$", "", "a\n", "", "null");
}

#[test]
fn node_029() {
    // JS: new RegExp("","").toString()
    check("source", "", "", "", "", "\"/(?:)/\"");
}

#[test]
fn node_030() {
    // JS: new RegExp("a/b","ig").toString()
    check("source", "a/b", "ig", "", "", "\"/a\\\\/b/gi\"");
}

#[test]
fn node_031() {
    // JS: new RegExp("a\\/b","").toString()
    check("source", "a\\/b", "", "", "", "\"/a\\\\/b/\"");
}

#[test]
fn node_032() {
    // JS: new RegExp("\n\r\u2028\u2029","").toString()
    check(
        "source",
        "\n\r  ",
        "",
        "",
        "",
        "\"/\\\\n\\\\r\\\\u2028\\\\u2029/\"",
    );
}

#[test]
fn node_033() {
    // JS: new RegExp("a","ymigsdu").toString()
    check("source", "a", "ymigsdu", "", "", "\"/a/dgimsuy\"");
}

#[test]
fn node_034() {
    // JS: new RegExp("a","v").toString()
    check("source", "a", "v", "", "", "\"/a/v\"");
}

#[test]
fn node_035() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=0;return Array.from({length:4},()=>{let m=r.exec("aba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "g",
        "aba",
        "0",
        "[[0,1],[2,3],[null,0],[0,1]]",
    );
}

#[test]
fn node_036() {
    // JS: (()=>{let r=new RegExp("a","y");r.lastIndex=0;return Array.from({length:4},()=>{let m=r.exec("aba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "y",
        "aba",
        "0",
        "[[0,1],[null,0],[0,1],[null,0]]",
    );
}

#[test]
fn node_037() {
    // JS: (()=>{let r=new RegExp("a","gy");r.lastIndex=0;return Array.from({length:4},()=>{let m=r.exec("ba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "gy",
        "ba",
        "0",
        "[[null,0],[null,0],[null,0],[null,0]]",
    );
}

#[test]
fn node_038() {
    // JS: (()=>{let r=new RegExp("a","");r.lastIndex=2;return Array.from({length:4},()=>{let m=r.exec("aba");return [m?m.index:null,r.lastIndex]})})()
    check("sequence", "a", "", "aba", "2", "[[0,2],[0,2],[0,2],[0,2]]");
}

#[test]
fn node_039() {
    // JS: (()=>{let r=new RegExp("","g");r.lastIndex=0;return Array.from({length:4},()=>{let m=r.exec("a");return [m?m.index:null,r.lastIndex]})})()
    check("sequence", "", "g", "a", "0", "[[0,0],[0,0],[0,0],[0,0]]");
}

#[test]
fn node_040() {
    // JS: (()=>{let r=new RegExp(".","gu");r.lastIndex=1;return Array.from({length:4},()=>{let m=r.exec("😀a");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        ".",
        "gu",
        "😀a",
        "1",
        "[[0,2],[2,3],[null,0],[0,2]]",
    );
}

#[test]
fn node_041() {
    // JS: (()=>{let r=new RegExp(".","y");r.lastIndex=1;return Array.from({length:4},()=>{let m=r.exec("😀");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        ".",
        "y",
        "😀",
        "1",
        "[[1,2],[null,0],[0,1],[1,2]]",
    );
}

#[test]
fn node_042() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=-3;return Array.from({length:4},()=>{let m=r.exec("ba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "g",
        "ba",
        "-3",
        "[[1,2],[null,0],[1,2],[null,0]]",
    );
}

#[test]
fn node_043() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=1.9;return Array.from({length:4},()=>{let m=r.exec("ba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "g",
        "ba",
        "1.9",
        "[[1,2],[null,0],[1,2],[null,0]]",
    );
}

#[test]
fn node_044() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=100;return Array.from({length:4},()=>{let m=r.exec("ba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "g",
        "ba",
        "100",
        "[[null,0],[1,2],[null,0],[1,2]]",
    );
}

#[test]
fn node_045() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=NaN;return Array.from({length:4},()=>{let m=r.exec("ba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "g",
        "ba",
        "NaN",
        "[[1,2],[null,0],[1,2],[null,0]]",
    );
}

#[test]
fn node_046() {
    // JS: (()=>{let r=new RegExp("a","g");r.lastIndex=Infinity;return Array.from({length:4},()=>{let m=r.exec("ba");return [m?m.index:null,r.lastIndex]})})()
    check(
        "sequence",
        "a",
        "g",
        "ba",
        "Infinity",
        "[[null,0],[1,2],[null,0],[1,2]]",
    );
}

#[test]
fn node_047() {
    // JS: "aba".match(new RegExp("a","g"))
    check("match", "a", "g", "aba", "", "[\"a\",\"a\"]");
}

#[test]
fn node_048() {
    // JS: "aba".match(new RegExp("z","g"))
    check("match", "z", "g", "aba", "", "null");
}

#[test]
fn node_049() {
    // JS: "😀".match(new RegExp("","g"))
    check("match", "", "g", "😀", "", "[\"\",\"\",\"\"]");
}

#[test]
fn node_050() {
    // JS: "😀".match(new RegExp("","gu"))
    check("match", "", "gu", "😀", "", "[\"\",\"\"]");
}
