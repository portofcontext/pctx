import json
from enum import IntEnum
from typing import Any, Literal, TypedDict

from pydantic import BaseModel
from typing_extensions import NotRequired

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


class ExecuteInput(BaseModel):
    code: str


class ExecuteOutput(BaseModel):
    """Output from executing TypeScript code"""

    success: bool
    stdout: str
    stderr: str
    output: Any | None = None

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


class ExecuteCodeRequest(JsonRpcBase):
    method: Literal["execute_code"]
    params: ExecuteCodeParams


class ExecuteCodeResponse(JsonRpcBase):
    result: ExecuteOutput


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
