import os
import re
from google import genai
from google.genai import types
import dotenv
import json
from .load_sys_prompt import get_sys_prompt

dotenv.load_dotenv()

client = genai.Client(api_key=os.getenv("GEMINI_API_KEY"))

chat = client.chats.create(
    model="gemini-3.8-flash",
    config=types.GenerateContentConfig(
        system_instruction = get_sys_prompt()
    )
)

def split_tools_from_text(text: str) -> tuple[str, list[dict[str, str]]]:
    """
    Detects attempted tool calls (valid JSON strings) in the text
    Splits output into a tuple of (text, list of tool calls)
    """
    tool_calls = []
    json_start_pattern = "```json\n"
    json_end_pattern = "```"
    json_starts = [m.start() for m in re.finditer(json_start_pattern, text)]
    print(f"json_starts: {json_starts}")
    json_ends = []
    for start in json_starts:
        # find the next valid end pattern
        end = text.find(json_end_pattern, start+len(json_start_pattern))
        if end != -1:
            json_ends.append(end)
    print(f"json_ends: {json_ends}")
    for i in range(min(len(json_starts), len(json_ends))):
        try:
            tool_call = json.loads(text[(json_starts[i]+len(json_start_pattern)):json_ends[i]])
            tool_calls.append(tool_call)
        except json.JSONDecodeError:
            continue
    # Remove the JSON blocks from the text
    for i in reversed(range(min(len(json_starts), len(json_ends)))):
        text = text[:json_starts[i]] + text[(json_ends[i]+len(json_end_pattern)):]
    return text, tool_calls

def invoke(prompt: str, context: str):
    """
    Invokes the Gemini API with the given prompt, context, and model.
    """
    response = chat.send_message(
        f"""
        User prompt:\n
        {prompt}\n
        Relevant context (previous tool call results):\n
        {context}
        """
    )
    return response