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
    print(text)
    print(tools)
    # Send init prompt to LLM and get text & tools
    # If no tools, send text to user and exit
    # If tools, hold text, send tools to MCP, get response, send it as context to LLM, repeat until done

    done = not tools

    while not done:
        # Main agentic loop
        