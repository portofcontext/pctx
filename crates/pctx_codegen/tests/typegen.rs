use pctx_codegen::{RootSchema, case::Case};
use pctx_type_check_runtime::type_check;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
struct TypegenTest {
    pub schema: RootSchema,
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
    let typegen_res = pctx_codegen::typegen::generate_types(test.schema, &type_name)
        .expect("Failed generating type");

    insta::assert_snapshot!(format!("{test_name}.ts"), &typegen_res.types);

    // run type checks
    for valid in &test.tests.valid {
        let typed_code = pctx_codegen::format::format_ts(&format!(
            "{types}\n\nconst value: {type_name} = {val};",
            types = typegen_res.types,
            val = valid.value
        ));

        let check_res = type_check(&typed_code).expect("failed typecheck");

        assert!(
            check_res.success,
            "valid test case id `{}` failed typecheck: {check_res:?}",
            valid.id
        );
    }

    for invalid in &test.tests.invalid {
        let typed_code = pctx_codegen::format::format_ts(&format!(
            "{types}\n\nconst value: {type_name} = {val};",
            types = typegen_res.types,
            val = invalid.value
        ));

        let check_res = type_check(&typed_code).expect("failed typecheck");

        assert!(
            !check_res.success,
            "invalid test case id `{}` succeeded typecheck (it should fail): {check_res:?}",
            invalid.id
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
    include_str!("./fixtures/typegen/basic_required.yml")
);
typegen_test!(
    test_basic_optional,
    include_str!("./fixtures/typegen/basic_optional.yml")
);
typegen_test!(test_union, include_str!("./fixtures/typegen/union.yml"));
typegen_test!(
    test_additional_properties,
    include_str!("./fixtures/typegen/additional_properties.yml")
);
typegen_test!(test_any, include_str!("./fixtures/typegen/any.yml"));
typegen_test!(test_enum, include_str!("./fixtures/typegen/enum.yml"));
typegen_test!(test_map, include_str!("./fixtures/typegen/map.yml"));
typegen_test!(
    test_optional_vs_nullable,
    include_str!("./fixtures/typegen/optional_vs_nullable.yml")
);
typegen_test!(
    test_circular_references,
    include_str!("./fixtures/typegen/circular_references.yml")
);
typegen_test!(
    test_recursive_non_object,
    include_str!("./fixtures/typegen/recursive_non_object.yml")
);
