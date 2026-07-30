async def fetch(url: str) -> str:
    result = await get(url)
    return result
