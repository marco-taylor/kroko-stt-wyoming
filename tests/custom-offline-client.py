#!/usr/bin/env python3
"""Host-side Wyoming client for an isolated test-container network namespace."""
import asyncio,json,pathlib,sys
from wyoming_native_test import transcribe
async def main():
    rows=[]
    for filename in sys.argv[1:]:
        x=await transcribe(10321,pathlib.Path(filename));assert x['hypothesis'].strip();rows.append(dict(audio=pathlib.Path(filename).name,**x))
    print(json.dumps(rows,ensure_ascii=False))
if __name__=='__main__':asyncio.run(main())
