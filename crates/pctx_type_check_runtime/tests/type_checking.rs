use pctx_type_check_runtime::{type_check, is_relevant_error, Diagnostic};

#[tokio::test]
async fn test_type_check_valid_code() {
    let code = r"const x: number = 42;";
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success);
    assert!(result.diagnostics.is_empty());
}

#[tokio::test]
async fn test_type_check_syntax_error() {
    let code = r"const x: number = ;";
    let result = type_check(code).await.expect("type check should not fail");
    assert!(!result.success);
    assert!(!result.diagnostics.is_empty());
}

#[test]
fn test_is_relevant_error_function() {
    // Relevant error (type mismatch TS2322)
    let relevant = Diagnostic {
        message: "Type 'string' is not assignable to type 'number'.".to_string(),
        line: Some(1),
        column: Some(1),
        severity: "error".to_string(),
        code: Some(2322),
    };
    assert!(is_relevant_error(&relevant), "TS2322 should be relevant");

    // Irrelevant error (module not found TS2307)
    let irrelevant_module = Diagnostic {
        message: "Cannot find module './foo'.".to_string(),
        line: Some(1),
        column: Some(1),
        severity: "error".to_string(),
        code: Some(2307),
    };
    assert!(
        !is_relevant_error(&irrelevant_module),
        "TS2307 should be irrelevant"
    );

    // Irrelevant error (require not found TS2304)
    let irrelevant_require = Diagnostic {
        message: "Cannot find name 'require'.".to_string(),
        line: Some(1),
        column: Some(1),
        severity: "error".to_string(),
        code: Some(2304),
    };
    assert!(
        !is_relevant_error(&irrelevant_require),
        "TS2304 should be irrelevant"
    );

    // Irrelevant error (implicit any TS7006)
    let irrelevant_implicit_any = Diagnostic {
        message: "Parameter implicitly has an 'any' type.".to_string(),
        line: Some(1),
        column: Some(1),
        severity: "error".to_string(),
        code: Some(7006),
    };
    assert!(
        !is_relevant_error(&irrelevant_implicit_any),
        "TS7006 should be irrelevant"
    );

    // Error without code should be relevant
    let no_code = Diagnostic {
        message: "Some error".to_string(),
        line: Some(1),
        column: Some(1),
        severity: "error".to_string(),
        code: None,
    };
    assert!(
        is_relevant_error(&no_code),
        "Errors without code should be relevant"
    );
}

#[tokio::test]
async fn test_for_of_loop() {
    let code = r#"
        for (const x of [1, 2, 3, 4]) {
            console.log(x);
        }
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "for...of should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_promises() {
    let code = r#"
        async function fetchData(): Promise<string> {
            return "hello";
        }
        const result: Promise<string> = fetchData();
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Promises should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_promise_all() {
    let code = r#"
        const promises = [Promise.resolve(1), Promise.resolve(2)];
        Promise.all(promises).then(values => {
            const sum: number = values[0] + values[1];
        });
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Promise.all should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_optional_chaining() {
    let code = r#"
        interface User {
            address?: { street?: string; };
        }
        function getStreet(user: User): string | undefined {
            return user.address?.street;
        }
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Optional chaining should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_nullish_coalescing() {
    let code = r#"
        const value: string | null = null;
        const result: string = value ?? "default";
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Nullish coalescing should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_bigint() {
    let code = r#"
        const big: bigint = 9007199254740991n;
        const sum: bigint = big + 1n;
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "BigInt should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_map_and_set() {
    let code = r#"
        const map = new Map<string, number>();
        map.set("a", 1);
        const val: number | undefined = map.get("a");

        const set = new Set<number>([1, 2, 3]);
        for (const item of set) {
            console.log(item);
        }
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Map and Set should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_array_methods() {
    let code = r#"
        const arr = [1, 2, 3, 4, 5];
        const flat = [[1, 2], [3, 4]].flat();
        const mapped = arr.flatMap(x => [x, x * 2]);
        const found = arr.find(x => x > 3);
        const includes = arr.includes(3);
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Array methods should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_async_iteration() {
    let code = r#"
        async function* generateNumbers(): AsyncGenerator<number> {
            yield 1;
            yield 2;
            yield 3;
        }
        async function consume() {
            for await (const num of generateNumbers()) {
                console.log(num);
            }
        }
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Async iteration should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_globalthis() {
    let code = r#"
        const g: typeof globalThis = globalThis;
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "globalThis should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_console_methods() {
    let code = r#"
        console.log("hello");
        console.error("error");
        console.warn("warning");
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Console methods should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_symbol() {
    let code = r#"
        const sym1 = Symbol("test");
        const sym2 = Symbol.for("global");
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "Symbol should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_weakmap_weakset() {
    let code = r#"
        const weakMap = new WeakMap<object, string>();
        const obj = {};
        weakMap.set(obj, "value");
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "WeakMap and WeakSet should pass. Diagnostics: {:?}", result.diagnostics);
}

#[tokio::test]
async fn test_type_error_still_caught() {
    let code = r#"
        const x: string = 42;
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(!result.success, "Type errors should still be caught");
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, Some(2322));
}

#[tokio::test]
async fn test_mcp_tools_still_available() {
    let code = r#"
        async function test() {
            const result = await callMCPTool({
                serverName: "test",
                toolName: "test_tool",
                arguments: { key: "value" }
            });
            return result;
        }
    "#;
    let result = type_check(code).await.expect("type check should not fail");
    assert!(result.success, "MCP tools should still be available. Diagnostics: {:?}", result.diagnostics);
}
