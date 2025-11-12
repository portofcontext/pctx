use codegen::case::Case;
use pctx_type_check_runtime::type_check;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct TypegenTest {
    pub schema: serde_json::Value,
    pub tests: SchemaTests,
}

#[derive(Debug, Clone, Deserialize)]
struct SchemaTests {
    #[serde(default)]
    pub valid: Vec<TestCase>,
    #[serde(default)]
    pub invalid: Vec<TestCase>,
}

#[derive(Debug, Clone, Deserialize)]
struct TestCase {
    pub id: String,
    pub value: serde_json::Value,
}

async fn run_typegen_test(test_name: &str, test: TypegenTest) {
    let type_name = Case::Pascal.sanitize(test_name.trim_start_matches("test_"));
    let typegen_res =
        codegen::typegen::generate_types(test.schema, &type_name).expect("Failed generating type");

    insta::assert_snapshot!(format!("{test_name}.ts"), &typegen_res.types);

    // run type checks
    for valid in &test.tests.valid {
        let typed_code = codegen::format::format_ts(&format!(
            "{types}\n\nconst value: {type_name} = {val};",
            types = typegen_res.types,
            val = valid.value
        ));

        let check_res = type_check(&typed_code).await.expect("failed typecheck");

        assert!(
            check_res.success,
            "valid test case id `{}` failed typecheck: {check_res:?}",
            valid.id
        );
    }
}

macro_rules! typegen_test {
    ($test_name:ident, $yml_str:expr) => {
        #[tokio::test]
        async fn $test_name() {
            let test: TypegenTest =
                serde_yaml::from_str($yml_str).expect("Failed to parse test YAML");
            run_typegen_test(stringify!($test_name), test).await;
        }
    };
}

typegen_test!(
    test_basic_required,
    include_str!("./fixtures/typegen/basic-required.yml")
);
typegen_test!(
    test_basic_optional,
    include_str!("./fixtures/typegen/basic-optional.yml")
);
