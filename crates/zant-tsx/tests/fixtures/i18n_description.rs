pub static DESCRIPTION: zant::i18n::catalog::Description = zant::i18n::catalog::Description {
    keys: &["escapes", "greeting", "items"],
    locales: &[
        zant::i18n::catalog::LocaleDesc {
            tag: "en",
            name: "English",
            plural: &[
                (zant::i18n::plural::Category::One, "i = 1 and v = 0 @integer 1"),
                (
                    zant::i18n::plural::Category::Other,
                    " @integer 0, 2~16, 100, 1000, 10000, 100000, 1000000, … @decimal 0.0~1.5, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0, …",
                ),
            ],
            messages: &[
                Some(
                    &[
                        zant::i18n::catalog::Op::Text(
                            "Quote: \" backslash: \\ newline:\n\t\r\u{0}\u{8}\u{7f}\u{85}\u{2028}\u{2029} 中",
                        ),
                    ],
                ),
                Some(
                    &[
                        zant::i18n::catalog::Op::Select {
                            arg: 1,
                            cases: &[
                                ("friend", &[zant::i18n::catalog::Op::Text("Hello")]),
                                ("other", &[zant::i18n::catalog::Op::Text("Welcome")]),
                            ],
                        },
                        zant::i18n::catalog::Op::Text(" "),
                        zant::i18n::catalog::Op::Number(0),
                        zant::i18n::catalog::Op::Text(" "),
                        zant::i18n::catalog::Op::Arg(1),
                    ],
                ),
                Some(
                    &[
                        zant::i18n::catalog::Op::Plural {
                            arg: 0,
                            exact: &[(0, &[zant::i18n::catalog::Op::Text("none")])],
                            categories: &[
                                (
                                    zant::i18n::plural::Category::One,
                                    &[
                                        zant::i18n::catalog::Op::Pound,
                                        zant::i18n::catalog::Op::Text(" item"),
                                    ],
                                ),
                                (
                                    zant::i18n::plural::Category::Other,
                                    &[
                                        zant::i18n::catalog::Op::Select {
                                            arg: 1,
                                            cases: &[
                                                (
                                                    "friend",
                                                    &[
                                                        zant::i18n::catalog::Op::Pound,
                                                        zant::i18n::catalog::Op::Text(" items for a friend"),
                                                    ],
                                                ),
                                                (
                                                    "other",
                                                    &[
                                                        zant::i18n::catalog::Op::Pound,
                                                        zant::i18n::catalog::Op::Text(" items"),
                                                    ],
                                                ),
                                            ],
                                        },
                                    ],
                                ),
                            ],
                        },
                    ],
                ),
            ],
        },
        zant::i18n::catalog::LocaleDesc {
            tag: "zh-CN",
            name: "简体中文",
            plural: &[
                (
                    zant::i18n::plural::Category::Other,
                    " @integer 0~15, 100, 1000, 10000, 100000, 1000000, … @decimal 0.0~1.5, 10.0, 100.0, 1000.0, 10000.0, 100000.0, 1000000.0, …",
                ),
            ],
            messages: &[
                None,
                Some(
                    &[
                        zant::i18n::catalog::Op::Arg(1),
                        zant::i18n::catalog::Op::Text("："),
                        zant::i18n::catalog::Op::Number(0),
                    ],
                ),
                Some(
                    &[
                        zant::i18n::catalog::Op::Plural {
                            arg: 0,
                            exact: &[(0, &[zant::i18n::catalog::Op::Text("没有")])],
                            categories: &[
                                (
                                    zant::i18n::plural::Category::One,
                                    &[
                                        zant::i18n::catalog::Op::Pound,
                                        zant::i18n::catalog::Op::Text(" 个"),
                                    ],
                                ),
                                (
                                    zant::i18n::plural::Category::Other,
                                    &[
                                        zant::i18n::catalog::Op::Select {
                                            arg: 1,
                                            cases: &[
                                                (
                                                    "friend",
                                                    &[
                                                        zant::i18n::catalog::Op::Text("朋友有 "),
                                                        zant::i18n::catalog::Op::Pound,
                                                        zant::i18n::catalog::Op::Text(" 个"),
                                                    ],
                                                ),
                                                (
                                                    "other",
                                                    &[
                                                        zant::i18n::catalog::Op::Pound,
                                                        zant::i18n::catalog::Op::Text(" 个"),
                                                    ],
                                                ),
                                            ],
                                        },
                                    ],
                                ),
                            ],
                        },
                    ],
                ),
            ],
        },
    ],
    default: 0,
};
