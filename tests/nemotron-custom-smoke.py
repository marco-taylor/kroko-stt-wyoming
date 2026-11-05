#!/usr/bin/env python3
"""CPU-only Nemotron E2E checks using an existing image and local binaries."""
import argparse
import asyncio
import hashlib
import json
import pathlib
import sys

from wyoming_native_test import Candidate, command, process_sample, transcribe


def fingerprint(root):
    result = {}
    for p in root.rglob('*'):
        if p.is_file():
            with p.open('rb') as f:
                result[str(p.relative_to(root))] = (hashlib.file_digest(f, 'sha256').hexdigest(), p.stat().st_mtime_ns)
    return result


async def main():
    p = argparse.ArgumentParser()
    p.add_argument('--image', required=True)
    p.add_argument('--binaries', type=pathlib.Path, required=True)
    p.add_argument('--models', type=pathlib.Path, required=True)
    p.add_argument('--audio', type=pathlib.Path, required=True)
    p.add_argument('--output', type=pathlib.Path, required=True)
    a = p.parse_args()
    a.output.mkdir(parents=True, exist_ok=False)
    before = fingerprint(a.models)
    result = {}
    for ms in (560, 1120):
        selected = f'custom/nemotron-{ms}'
        c = Candidate(a.image, a.binaries, a.models, selected, a.output / str(ms), debug=True)
        c.start()
        try:
            description = await c.describe()
            assert description['asr'][0]['models'][0]['languages'] == ['de', 'de-DE']
            assert c.info['Config']['User'] == '99:100'
            rows = []
            for name in ('01.wav', '03.wav'):
                row = await transcribe(c.port, a.audio/name, paced=True)
                assert row['hypothesis'].strip()
                rows.append(dict(audio=name, **row))
            try:
                await transcribe(c.port, a.audio/'01.wav', language='en')
            except RuntimeError as e:
                assert 'unsupported_language' in str(e)
            else:
                raise AssertionError('Wrong language accepted')
            c.restart()
            restart = await transcribe(c.port, a.audio/'01.wav')
            result[str(ms)] = dict(startup_s=c.startup_s, after_load=process_sample(c.pid),
                                   describe=description, paced_transcriptions=rows,
                                   restart=restart, wrong_language_rejected=True)
        finally:
            c.close()
        # Networkless startup and model verification; no source or cache writes.
        c = Candidate(a.image, a.binaries, a.models, selected, a.output/(str(ms)+'-offline'), offline=True)
        c.start()
        try:
            result[str(ms)]['offline_describe'] = command(['docker', 'exec', c.name, '/usr/local/bin/wyoming-probe']).returncode == 0
        finally:
            c.close()
    assert fingerprint(a.models) == before
    result['model_files_unchanged'] = True
    result['all_passed'] = True
    (a.output/'results.json').write_text(json.dumps(result, indent=2))
    print(json.dumps(result, indent=2), flush=True)


if __name__ == '__main__':
    asyncio.run(main())
