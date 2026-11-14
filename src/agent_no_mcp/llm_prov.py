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
    tool_call_pos: list[tuple[int, int]] = []
    # Find all JSON objects in the text
    decoder = json.JSONDecoder()
    pos = 0
    while pos < len(text):
        match = re.search(r'\{', text[pos:])
        if not match:
            break
        start = pos + match.start()
        try:
            obj, end = decoder.raw_decode(text[start:])
            tool_calls.append(obj)
            tool_call_pos.append((start, start + end))
            pos = start + end
        except json.JSONDecodeError:
            pos = start + 1
    # remove calls from text
    for start, end in reversed(tool_call_pos):
        text = text[:start] + text[end:]
    return text, tool_calls

def invoke(prompt: str, context: str):
    """
    Invokes the Gemini API with the given prompt, context, and model.
    """
    response = chat.send_message(
        f"""
        User prompt:\n
        {prompt}\n
        Relevant context:\n
        {context}
        """
    )
    return response