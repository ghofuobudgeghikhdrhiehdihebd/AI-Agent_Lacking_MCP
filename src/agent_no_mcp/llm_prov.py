from google import genai
import dotenv
import os

dotenv.load_dotenv()

gemini_client = genai.Client(api_key=os.getenv("GEMINI_API_KEY"))

