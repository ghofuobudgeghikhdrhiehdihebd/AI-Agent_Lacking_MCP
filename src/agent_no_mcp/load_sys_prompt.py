with open("src/agent_no_mcp/sys_prompt_template.md", "r") as f:
    sys_prompt = f.read()

def get_sys_prompt():
    return sys_prompt