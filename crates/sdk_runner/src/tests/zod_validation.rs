use crate::*;

#[tokio::test]
async fn test_execute_with_zod_valid() {
    let code = r#"
import { z } from "zod";

const schema = z.object({
    name: z.string(),
    age: z.number(),
});

const data = { name: "Alice", age: 30 };
const result = schema.parse(data);

export default result;
"#;

    let result = execute(code).await.expect("execution should succeed");
    assert!(result.success, "Valid Zod parse should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
    assert!(result.diagnostics.is_empty(), "Should have no type errors");
}

#[tokio::test]
async fn test_execute_with_zod_invalid() {
    let code = r#"
import { z } from "zod";

const schema = z.object({
    name: z.string(),
    age: z.number(),
});

const data = { name: "Alice", age: "thirty" };
const result = schema.parse(data);

export default result;
"#;

    let result = execute(code).await.expect("execution should succeed");
    assert!(!result.success, "Invalid Zod parse should fail");
    assert!(result.runtime_error.is_some(), "Should have runtime error");

    let error = result.runtime_error.unwrap();
    assert!(
        error.message.contains("ZodError")
            || error.message.contains("validation")
            || error.message.contains("Expected number"),
        "Error should mention Zod validation failure, got: {}",
        error.message
    );
}

#[tokio::test]
async fn test_execute_with_zod_safe_parse() {
    let code = r#"
import { z } from "zod";

const schema = z.object({
    name: z.string(),
    age: z.number(),
});

const data = { name: "Alice", age: "thirty" };
const result = schema.safeParse(data);

export default result;
"#;

    let result = execute(code).await.expect("execution should succeed");
    assert!(result.success, "safeParse should not throw");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
}

#[tokio::test]
async fn test_execute_with_zod_complex_schema() {
    let code = r#"
import { z } from "zod";

const userSchema = z.object({
    username: z.string().min(3),
    email: z.string().email(),
    age: z.number().min(18),
    roles: z.array(z.enum(["admin", "user", "guest"])),
});

const validUser = {
    username: "alice123",
    email: "alice@example.com",
    age: 25,
    roles: ["user", "admin"]
};

const result = userSchema.parse(validUser);
export default result;
"#;

    let result = execute(code).await.expect("execution should succeed");
    assert!(result.success, "Complex valid schema should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
}

#[tokio::test]
async fn test_execute_with_zod_transform() {
    let code = r#"
import { z } from "zod";

const schema = z.string().transform((val) => val.toUpperCase());
const result = schema.parse("hello");

export default result;
"#;

    let result = execute(code).await.expect("execution should succeed");
    assert!(result.success, "Schema with transform should succeed");
    assert!(
        result.runtime_error.is_none(),
        "Should have no runtime errors"
    );
}
