"""
This is a simulation of a remote MCP server
It performs input validation, but it does not call any tools
Instead, it passes tool calls to the airlock via USB
"""

def mcp_server(tool: str, params: dict[str, dict[str, str]]) -> dict[str, str]:
    """
    Simulates a remote MCP server that performs input validation and passes tool calls to the airlock via USB.
    """
    # Perform input validation
    if not isinstance(tool, str) or not isinstance(params, dict):
        return {"error": "Invalid input types"}

    # TODO: pass tool calls to the airlock here
    # TODO: get response from airlock and return it to the agent

    # Sample responses below
    sample_ok_response = {"status": "success", "result": "This is a test response indicating that the tool was successfully called"}
    sample_tool_failure_response = {"status": "failed", "error": "(tool failure)"}
    sample_airlock_failure_response = {"status": "failed", "error": "permission denied"}

    return sample_ok_response # TODO: switch for actual response