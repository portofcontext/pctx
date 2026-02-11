"""
PCTX WebSocket Client

Connects to a PCTX WebSocket server to register Python tool callbacks
and execute TypeScript code.
"""

import asyncio
import json
import uuid
from typing import Any, Union

import pydantic
import websockets
from websockets.asyncio.client import ClientConnection

from pctx_client._tool import AsyncTool, Tool
from pctx_client.models import (
    ErrorCode,
    ErrorData,
    ExecuteCodeParams,
    ExecuteCodeRequest,
    ExecuteCodeResponse,
    ExecuteOutput,
    ExecuteToolRequest,
    ExecuteToolResponse,
    ExecuteToolResult,
    JsonRpcError,
)

from .exceptions import ConnectionError

WebSocketMessage = Union[
    ExecuteCodeRequest,
    ExecuteCodeResponse,
    ExecuteToolRequest,
    ExecuteToolResponse,
    JsonRpcError,
]


class WebSocketClient:
    """
    PCTX WebSocket Client

    Connects to a PCTX WebSocket server, allowing you to
    receive and handle tool execution requests from the server
    """

    def __init__(
        self,
        url: str,
        api_key: str | None = None,
        tools: list[Tool | AsyncTool] | None = None,
    ):
        """
        Initialize the WebSocket client.

        Args:
            url: WebSocket server URL (e.g., "ws://localhost:8080/ws")
        """
        self.url = url
        self.ws: ClientConnection | None = None
        self.tools = tools or []
        self._api_key = api_key
        self._pending_executions: dict[str | int, asyncio.Future] = {}
        self._request_counter = 0

    async def _connect(self, code_mode_session: str):
        """
        Connect to the WebSocket server.

        Raises:
            ConnectionError: If connection fails
        """
        try:
            headers = {
                "x-code-mode-session": code_mode_session,
                "x-pctx-api-key": self._api_key,
            }
            self.ws = await websockets.connect(self.url, additional_headers=headers)
        except Exception as e:
            raise ConnectionError(f"Failed to connect to {self.url}: {e}") from e

        # Start message handler after receiving session_created
        self._message_handler_task = asyncio.create_task(self._handle_messages())

    async def _disconnect(self):
        """Disconnect from the WebSocket server."""
        if self._message_handler_task:
            self._message_handler_task.cancel()

        if self.ws:
            await self.ws.close()
            self.ws = None

    async def _send(self, message: WebSocketMessage):
        """
        Send a message via the websocket
        """
        if self.ws is None:
            raise ConnectionError(
                "Cannot send messages when websocket is not connected"
            )

        await self.ws.send(message.model_dump_json())

    async def execute_code(
        self, code_mode_session: str, code: str, timeout: float = 30.0
    ) -> ExecuteOutput:
        """
        Execute code via WebSocket instead of REST.

        Args:
            code_mode_session: CodeMode session to run execution in
            code: TypeScript/JavaScript code to execute
            timeout: Timeout in seconds (default 30)

        Returns:
            ExecuteOutput with success, stdout, stderr, and output

        Raises:
            TimeoutError: If execution times out
            Exception: If execution fails
        """
        if self.ws is None:
            await self._connect(code_mode_session)

        # Generate unique request ID
        request_id = str(uuid.uuid4())

        # Create future for response
        future: asyncio.Future[dict[str, Any]] = asyncio.Future()
        self._pending_executions[request_id] = future

        # Send request
        request = ExecuteCodeRequest(
            id=request_id, method="execute_code", params=ExecuteCodeParams(code=code)
        )

        try:
            await self._send(request)

            # Wait for response with timeout
            result = await asyncio.wait_for(future, timeout=timeout)
            return ExecuteOutput.model_validate(result)
        except asyncio.TimeoutError:
            self._pending_executions.pop(request_id, None)
            raise TimeoutError(f"Code execution timed out after {timeout}s")
        finally:
            self._pending_executions.pop(request_id, None)
            await self._disconnect()

    async def _handle_messages(self):
        """Background task to handle incoming WebSocket messages."""
        if self.ws is None:
            raise ConnectionError(
                "Cannot handle messages when websocket is not connected"
            )

        try:
            async for message_data in self.ws:
                try:
                    adapter = pydantic.TypeAdapter(WebSocketMessage)
                    message: WebSocketMessage = adapter.validate_json(message_data)

                    if isinstance(message, ExecuteToolRequest):
                        res = await self._handle_execute_tool(message)
                        await self._send(res)
                    elif isinstance(message, ExecuteCodeResponse):
                        future = self._pending_executions.get(message.id)
                        if future is not None:
                            future.set_result(message.result)
                    elif isinstance(message, JsonRpcError):
                        future = self._pending_executions.get(message.id)
                        if future is not None:
                            future.set_exception(
                                Exception(f"Execution error: {message.error.message}")
                            )

                except pydantic.ValidationError as e:
                    print(f"Failed to decode message: {message_data} - {e}")
                except json.JSONDecodeError as e:
                    print(f"Failed to parse JSON: {message_data} - {e}")
                except Exception as e:
                    print(f"Error processing message: {e}")
        except asyncio.CancelledError:
            pass
        except Exception as e:
            print(f"Message handler error: {e}")

    async def _handle_execute_tool(
        self, req: ExecuteToolRequest
    ) -> ExecuteToolResponse | JsonRpcError:
        # Find tool to execute
        tool = next(
            (
                t
                for t in self.tools
                if t.name == req.params.name and t.namespace == req.params.namespace
            ),
            None,
        )
        if tool is None:
            return JsonRpcError(
                id=req.id,
                error=ErrorData(
                    code=ErrorCode.METHOD_NOT_FOUND,
                    message=f"No tool `{req.params.name}` exists in namespace `{req.params.namespace}`",
                ),
            )

        args = req.params.args or {}
        try:
            if isinstance(tool, Tool):
                if tool.input_schema is None:
                    output = tool.invoke()
                else:
                    output = tool.invoke(**args)
            else:
                if tool.input_schema is None:
                    output = await tool.ainvoke()
                else:
                    output = await tool.ainvoke(**args)

            return ExecuteToolResponse(
                id=req.id, result=ExecuteToolResult(output=output)
            )
        except pydantic.ValidationError as e:
            return JsonRpcError(
                id=req.id,
                error=ErrorData(
                    code=ErrorCode.INVALID_PARAMS,
                    message=f"Failed validating tool params: {e}",
                ),
            )
        except Exception as e:
            return JsonRpcError(
                id=req.id,
                error=ErrorData(
                    code=ErrorCode.INTERNAL_ERROR,
                    message=f"Failed executing tool: {e}",
                ),
            )
