import json
import warnings
from datetime import datetime, timedelta, timezone
from enum import Enum, IntEnum
from typing import Annotated, Any, Literal, TypedDict

from pydantic import BaseModel, BeforeValidator, Field
from typing_extensions import NotRequired

from pctx_client._utils import HAS_SEARCH

ToolName = Literal[
    "execute_bash",
    "execute_typescript",
    "get_function_details",
    "list_functions",
    "search_functions",
]


ToolDisclosureName = Literal["catalog", "filesystem"]


class ToolDisclosure(str, Enum):
    CATALOG = "catalog"
    FS = "filesystem"
    # SIDECAR = "sidecar" # <--- not fully supported by session server yet

    def contains_tool(self, tool_name: ToolName) -> bool:
        if self == ToolDisclosure.CATALOG:
            allowed = {
                "get_function_details",
                "list_functions",
                "execute_typescript",
            }
            if HAS_SEARCH:
                allowed.add("search_functions")
            return tool_name in allowed
        elif self == ToolDisclosure.FS:
            return tool_name in {
                "execute_bash",
                "execute_typescript",
            }
        # elif self == DisclosureStyle.SIDECAR:  # <--- not fully supported by session server yet
        #     return tool_name == "execute_typescript"
        else:
            raise ValueError(f"Unhandled ToolDisclosure variant: {self}")


# ------------- Tool Callback Config ------------


class ToolConfig(TypedDict):
    name: str
    namespace: str
    description: NotRequired[str]
    input_schema: NotRequired[dict[str, Any] | None]
    output_schema: NotRequired[dict[str, Any] | None]


# -------------- MCP Server Config --------------


class BearerAuth(TypedDict):
    """Bearer token authentication"""

    type: Literal["bearer"]
    token: str


class HeadersAuth(TypedDict):
    """Custom headers authentication"""

    type: Literal["headers"]
    headers: dict[str, str]


class HttpServerConfig(TypedDict):
    """Configuration for an HTTP MCP server connection"""

    name: str
    url: str
    auth: NotRequired[BearerAuth | HeadersAuth]


class StdioServerConfig(TypedDict):
    """Configuration for a stdio MCP server connection"""

    name: str
    command: str
    args: NotRequired[list[str]]
    env: NotRequired[dict[str, str]]


ServerConfig = HttpServerConfig | StdioServerConfig


# -------------- Code Mode Outputs --------------


class ListedFunction(BaseModel):
    """Represents a listed function with basic metadata"""

    namespace: str
    name: str
    description: str | None = None


class ListFunctionsOutput(BaseModel):
    """Output from listing available functions"""

    functions: list[ListedFunction]
    code: str


class FunctionDetails(BaseModel):
    """Detailed information about a function including types"""

    namespace: str
    name: str
    description: str | None = None
    input_type: str
    output_type: str
    types: str


class GetFunctionDetailsInput(BaseModel):
    functions: list[str]


class GetFunctionDetailsOutput(BaseModel):
    """Output from getting detailed function information"""

    functions: list[FunctionDetails]
    code: str


class ExecuteTypescriptInput(BaseModel):
    code: str


class ExecuteBashInput(BaseModel):
    command: str


class _SystemTime(BaseModel):
    secs_since_epoch: int
    nanos_since_epoch: int


def _parse_system_time(value: Any) -> datetime:
    if isinstance(value, dict):
        st = _SystemTime.model_validate(value)
        return datetime(1970, 1, 1, tzinfo=timezone.utc) + timedelta(
            seconds=st.secs_since_epoch,
            microseconds=st.nanos_since_epoch // 1000,
        )
    return value


# Rust SystemTime deserializes to a UTC datetime (microsecond precision; sub-µs nanoseconds truncated)
SystemTime = Annotated[datetime, BeforeValidator(_parse_system_time)]


class Diagnostic(BaseModel):
    """A single TypeScript type-checking diagnostic"""

    message: str
    line: int | None = None
    column: int | None = None
    severity: str
    code: int | None = None


class TypecheckOutcomePassed(BaseModel):
    type: Literal["passed"]

    def is_success(self) -> bool:
        return True


class TypecheckOutcomeFailed(BaseModel):
    type: Literal["failed"]
    diagnostics: list[Diagnostic]

    def is_success(self) -> bool:
        return False


TypecheckOutcome = Annotated[
    TypecheckOutcomePassed | TypecheckOutcomeFailed,
    Field(discriminator="type"),
]


class TypeCheckEvent(BaseModel):
    type: Literal["type_check"]
    started_at: SystemTime
    ended_at: SystemTime
    outcome: TypecheckOutcome


class EventOutcomeSuccess(BaseModel):
    type: Literal["success"]
    output: Any | None = None

    def is_success(self) -> bool:
        return True


class EventOutcomeError(BaseModel):
    type: Literal["error"]
    message: str

    def is_success(self) -> bool:
        return False


EventOutcome = Annotated[
    EventOutcomeSuccess | EventOutcomeError,
    Field(discriminator="type"),
]


class McpToolCallEvent(BaseModel):
    type: Literal["mcp_tool_call"]
    server: str
    tool: str
    cached_client: bool
    args: Any | None = None
    outcome: EventOutcome
    started_at: SystemTime
    ended_at: SystemTime


class CallbackInvocationEvent(BaseModel):
    type: Literal["callback_invocation"]
    id: str
    args: Any | None = None
    outcome: EventOutcome
    started_at: SystemTime
    ended_at: SystemTime


ExecutionEvent = Annotated[
    TypeCheckEvent | McpToolCallEvent | CallbackInvocationEvent,
    Field(discriminator="type"),
]


class ExecutionTrace(BaseModel):
    code: str
    started_at: SystemTime
    ended_at: SystemTime
    events: list[ExecutionEvent]


class ExecuteTypescriptOutput(BaseModel):
    """Output from executing TypeScript code"""

    success: bool
    stdout: str
    stderr: str
    output: Any | None = None
    trace: ExecutionTrace

    def markdown(self) -> str:
        return f"""Code Executed Successfully: {self.success}

# Return Value
```json
{json.dumps(self.output)}
```

# STDOUT
{self.stdout}

# STDERR
{self.stderr}
"""


class ExecuteBashOutput(BaseModel):
    """Output from executing bash command"""

    exit_code: int
    stdout: str
    stderr: str

    def markdown(self) -> str:
        return f"""Exit Code: {self.exit_code}

# STDOUT
{self.stdout}

# STDERR
{self.stderr}
"""


# -------------- Websocket jsonrpc Messages --------------
class JsonRpcBase(BaseModel):
    jsonrpc: Literal["2.0"] = "2.0"
    id: str | int


class ErrorCode(IntEnum):
    RESOURCE_NOT_FOUND = -32002
    INVALID_REQUEST = -32600
    METHOD_NOT_FOUND = -32601
    INVALID_PARAMS = -32602
    INTERNAL_ERROR = -32603
    PARSE_ERROR = -32700


class ErrorData(BaseModel):
    code: ErrorCode
    message: str
    data: dict[str, Any] | None = None


class JsonRpcError(JsonRpcBase):
    error: ErrorData


class ExecuteCodeParams(BaseModel):
    code: str
    disclosure: ToolDisclosure = ToolDisclosure.CATALOG


class ExecuteCodeRequest(JsonRpcBase):
    method: Literal["execute_typescript"]
    params: ExecuteCodeParams


class ExecuteCodeResponse(JsonRpcBase):
    result: ExecuteTypescriptOutput


class ExecuteToolParams(BaseModel):
    namespace: str
    name: str
    args: dict[str, Any] | None


class ExecuteToolRequest(JsonRpcBase):
    method: Literal["execute_tool"]
    params: ExecuteToolParams


class ExecuteToolResult(BaseModel):
    output: Any | None


class ExecuteToolResponse(JsonRpcBase):
    result: ExecuteToolResult


# -------------- Deprecation warnings & backwards compat classes --------------


def __getattr__(name: str) -> object:
    if name == "ExecuteInput":
        warnings.warn(
            "ExecuteInput is deprecated, use ExecuteTypescriptInput instead",
            DeprecationWarning,
            stacklevel=2,
        )
        return ExecuteTypescriptInput
    if name == "ExecuteOutput":
        warnings.warn(
            "ExecuteOutput is deprecated, use ExecuteTypescriptOutput instead",
            DeprecationWarning,
            stacklevel=2,
        )
        return ExecuteTypescriptOutput
    raise AttributeError(f"module {__name__!r} has no attribute {name!r}")
