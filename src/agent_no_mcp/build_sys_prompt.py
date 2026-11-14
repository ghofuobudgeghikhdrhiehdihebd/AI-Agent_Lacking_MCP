import json
import yaml
import textwrap

def build_sys_prompt():
    with open("src/agent_no_mcp/sys_prompt_template.md", "r") as f:
        sys_prompt_lines = f.readlines()
        # strip final newline
        sys_prompt_lines = [line.rstrip("\n") for line in sys_prompt_lines]
    # Look for the key lines
    tool_desc_line = sys_prompt_lines.index("<tools>")+1
    tool_defs_line = -1 # end of file
    with open("src/agent_no_mcp/tools.json", "r") as f:
        tools = json.load(f)
    # Build the tool description and definition strings
    tool_desc_builder = []
    tool_defs_builder = []
    for tool, data in tools.items():
        tool_desc_builder.append(f"{tool}: {data['description']}")
        tool_defs_builder.append(f"{tool}:\n{textwrap.indent(yaml.safe_dump(data, indent=2), '  ')}")
    
    tool_desc_str = "\n".join(tool_desc_builder)
    tool_defs_str = "\n".join(tool_defs_builder)

    sys_prompt_lines[tool_desc_line] = tool_desc_str
    sys_prompt_lines[tool_defs_line] = tool_defs_str

    with open("src/agent_no_mcp/sys_prompt.md", "w") as f:
        f.write("\n".join(sys_prompt_lines))

if __name__ == "__main__":
    build_sys_prompt()