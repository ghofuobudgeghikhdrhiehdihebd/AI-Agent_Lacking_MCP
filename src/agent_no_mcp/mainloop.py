import sys
from .llm_prov import invoke, split_tools_from_text
from .mcp_serv import mcp_server

def main():
    init_prompt = sys.argv[1]
    context = []

    init_res = invoke(init_prompt, "".join(context))
    if (init_res.text is None):
        return ""
    text, tools = split_tools_from_text(init_res.text)
    print(f"Response (text): {text}\nResponse (tools): {tools}\n")

    while tools:
        next_tool = tools.pop(0)
        tool_name = next_tool.get("tool", "INVALID_TOOL")
        tool_params = next_tool.get("params", {})
        mcp_res = mcp_server(tool_name, tool_params)
        if (mcp_res.get("status") == "success"):
            context.append(f"Tool {tool_name} executed successfully with result: {mcp_res.get('result', '')}")
        else:
            context.append(f"Tool {tool_name} failed with error: {mcp_res.get('error', 'Unknown error')}")
        res = invoke(init_prompt, "".join(context))
        if (res.text is None):
            continue
        text, add_tools = split_tools_from_text(res.text)
        print(f"Response (text): {text}\nResponse (tools): {add_tools}\n")
        tools.extend(add_tools)
        print(f"Current context: {context}\n")