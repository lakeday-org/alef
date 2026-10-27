use super::bridge_fields::gen_bridge_field_wrapper_function;
use crate::codegen::generators::trait_bridge::BridgeFieldMatch;
use crate::core::config::TraitBridgeConfig;
use crate::core::ir::{FieldDef, FunctionDef, ParamDef, PrimitiveType, TypeRef};
use std::collections::HashSet;

fn wrapper(optional: bool) -> String {
    let bridge = TraitBridgeConfig {
        trait_name: "Visitor".into(),
        ..Default::default()
    };
    let field = FieldDef {
        name: "visitor".into(),
        ..Default::default()
    };
    let matched = BridgeFieldMatch {
        param_index: 2,
        param_name: "options".into(),
        options_type: "Options".into(),
        param_is_optional: true,
        field_name: "visitor".into(),
        field: &field,
        bridge: &bridge,
    };
    let func = FunctionDef {
        name: "apply_policy".into(),
        return_type: TypeRef::Unit,
        params: vec![
            param("enabled", TypeRef::Primitive(PrimitiveType::Bool), optional),
            param("limit", TypeRef::Primitive(PrimitiveType::U32), optional),
            param("options", TypeRef::Named("Options".into()), true),
        ],
        ..Default::default()
    };
    let empty = HashSet::new();
    gen_bridge_field_wrapper_function(&func, &matched, "SampleException", &empty, &empty, &empty)
}

fn param(name: &str, ty: TypeRef, optional: bool) -> ParamDef {
    ParamDef {
        name: name.into(),
        ty,
        optional,
        ..Default::default()
    }
}

#[test]
fn bridge_scalar_arguments_use_native_marshalling_in_both_paths() {
    for optional in [false, true] {
        let code = wrapper(optional);
        let expected = if optional {
            "NativeMethods.ApplyPolicy(((enabled ?? false) ? 1 : 0), limit ?? uint.MaxValue, optionsHandle)"
        } else {
            "NativeMethods.ApplyPolicy((enabled ? 1 : 0), limit, optionsHandle)"
        };
        assert_eq!(code.matches(expected).count(), 2, "{code}");
        assert!(!code.contains("NativeMethods.ApplyPolicy(enabled,"));
    }
}

#[test]
#[ignore = "requires a runnable .NET SDK; run explicitly for C# bridge changes"]
fn bridge_scalar_generated_csharp_executes_required_and_optional_policies() {
    let directory = tempfile::tempdir().expect("isolated C# project");
    let version = crate::test_support::spawn_from_stable_dir("dotnet")
        .arg("--version")
        .output()
        .expect("query .NET SDK");
    assert!(version.status.success(), "a runnable .NET SDK is required");
    let version = String::from_utf8(version.stdout).expect("UTF-8 SDK version");
    let major = version.trim().split('.').next().expect("SDK major version");
    let project = format!(
        "<Project Sdk=\"Microsoft.NET.Sdk\"><PropertyGroup><OutputType>Exe</OutputType>\
         <TargetFramework>net{major}.0</TargetFramework><Nullable>enable</Nullable>\
         <TreatWarningsAsErrors>true</TreatWarningsAsErrors></PropertyGroup></Project>"
    );
    std::fs::write(directory.path().join("Probe.csproj"), project).expect("write project");
    let source = CSHARP_HARNESS.replace("GENERATED_METHODS", &(wrapper(false) + &wrapper(true)));
    std::fs::write(directory.path().join("Program.cs"), source).expect("write generated wrapper probe");
    let output = std::process::Command::new("dotnet")
        .args(["run", "--nologo", "--verbosity", "quiet"])
        .current_dir(directory.path())
        .env("DOTNET_CLI_HOME", directory.path().join(".dotnet"))
        .output()
        .expect("run generated C# wrappers");
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(String::from_utf8_lossy(&output.stdout).contains("POLICIES:10 BRIDGES:5"));
}

const CSHARP_HARNESS: &str = r#"
using System;
using System.Collections.Generic;
using System.Text.Json;
sealed class Options { public object? Visitor { get; set; } }
sealed class VisitorBridge(object visitor) : IDisposable {
    internal static readonly object _registryLock = new();
    internal static readonly Dictionary<int, VisitorBridge> _bridgeRegistry = new();
    internal readonly int _bridgeId = 1;
    internal readonly object _vtable = visitor;
    public void Dispose() { }
}
static class NativeMethods {
    internal static int Expected;
    internal static uint Limit;
    internal static int Calls;
    internal static int Bridges;
    internal static ulong OptionsFromJson(string json) => 1;
    internal static void OptionsFree(ulong handle) { }
    internal static ulong VisitorBridgeNew(object table, int id) => 2;
    internal static void VisitorBridgeFree(ulong handle) { }
    internal static void OptionsSetVisitor(ulong options, ulong bridge) { Bridges++; }
    internal static void ApplyPolicy(int enabled, uint limit, ulong options) {
        if (enabled != Expected || limit != Limit || options != 1)
            throw new InvalidOperationException("wrong native scalar arguments");
        Calls++;
    }
}
static class Converter {
    private static readonly JsonSerializerOptions JsonSerializationOptions = new();
    private static Exception GetLastError() => new InvalidOperationException();
GENERATED_METHODS
}
static class Program {
    static void Main() {
        foreach (bool visitor in new[] { false, true }) {
            var options = new Options { Visitor = visitor ? new object() : null };
            foreach (bool enabled in new[] { false, true }) {
                NativeMethods.Expected = enabled ? 1 : 0;
                NativeMethods.Limit = 7;
                Converter.ApplyPolicy(enabled, 7, options);
                Converter.ApplyPolicy((bool?)enabled, (uint?)7, options);
            }
            NativeMethods.Expected = 0;
            NativeMethods.Limit = uint.MaxValue;
            Converter.ApplyPolicy(null, null, options);
        }
        if (NativeMethods.Calls != 10 || NativeMethods.Bridges != 5)
            throw new InvalidOperationException("incorrect executed branch count");
        Console.WriteLine($"POLICIES:{NativeMethods.Calls} BRIDGES:{NativeMethods.Bridges}");
    }
}
"#;
