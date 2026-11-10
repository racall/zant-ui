//! Every golden output of the TSX compiler compiles against zant
//! (`tools/tsx/testdata`); the compiler's own tests check it is what the
//! compiler produces.
const std = @import("std");

/// References a generated file's component struct and all its methods
/// (Zig only analyzes what is referenced).
fn refComponent(comptime M: type) void {
    inline for (comptime std.meta.declarations(M)) |name| {
        const decl = @field(M, name);
        if (@TypeOf(decl) == type and @typeInfo(decl) == .@"struct") std.testing.refAllDecls(decl);
    }
}

test "tsx golden outputs compile" {
    refComponent(@import("tsx_golden_counter"));
    refComponent(@import("tsx_golden_layout"));
    refComponent(@import("tsx_golden_text"));
    refComponent(@import("tsx_golden_events"));
    refComponent(@import("tsx_golden_conditional"));
    refComponent(@import("tsx_golden_components"));
    refComponent(@import("tsx_golden_todos"));
    refComponent(@import("tsx_golden_arrays"));
    refComponent(@import("tsx_golden_children"));
    refComponent(@import("tsx_golden_theme"));
    refComponent(@import("tsx_golden_list"));
    refComponent(@import("tsx_golden_local_state"));
    refComponent(@import("tsx_golden_hooks"));
    refComponent(@import("tsx_golden_store"));
    refComponent(@import("tsx_golden_nesting"));
    refComponent(@import("tsx_golden_strings"));
    refComponent(@import("tsx_golden_widgets"));
    refComponent(@import("tsx_golden_functions"));
    refComponent(@import("tsx_golden_loops"));
    refComponent(@import("tsx_golden_spread"));
}
